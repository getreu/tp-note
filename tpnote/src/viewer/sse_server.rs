//! Server-sent-event server for the note viewer feature.
//! This module contains also the web browser Javascript client code.

use crate::config::CFG;
use crate::viewer::error::ViewerError;
use crate::viewer::http_response::HttpResponse;
use crate::viewer::init::LOCALHOST;
use parking_lot::RwLock;
use percent_encoding::percent_decode_str;
use std::collections::HashSet;
use std::io::{ErrorKind, Read, Write};
use std::net::Ipv4Addr;
use std::net::SocketAddr;
use std::net::SocketAddrV4;
use std::net::{TcpListener, TcpStream};
use std::path::PathBuf;
use std::str;
use std::sync::mpsc::{sync_channel, Receiver, SyncSender};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::SystemTime;
use tpnote_lib::config::TMPL_HTML_VAR_VIEWER_DOC_JS;
use tpnote_lib::context::Context;

/// The TCP stream is read in chunks. This is the read buffer size.
const TCP_READ_BUFFER_SIZE: usize = 0x400;

/// Javascript client code, part 1
/// Refresh on WTFiles events.
pub const SSE_CLIENT_CODE1: &str = r#"
    var evtSource = new EventSource("http://"#;
/// Javascript client code, part 2
/// Save last scroll position into local storage.
/// Jump to the last saved scroll position.
pub const SSE_CLIENT_CODE2: &str = r#"/events");
    evtSource.addEventListener("update", function(e) {
        localStorage.setItem('scrollPosition', window.scrollY);
        window.location.reload(true);
    });
    window.addEventListener('load', function() {
        if(localStorage.getItem('scrollPosition') !== null)
            window.scrollTo(0, localStorage.getItem('scrollPosition'));
    });
    "#;

/// URL path for Server-Sent-Events.
const SSE_EVENT_PATH: &str = "/events";

/// Server-Sent-Event tokens our HTTP client has registered to receive.
#[derive(Debug, Clone, Copy)]
pub enum SseToken {
    /// Server-Sent-Event token to request nothing but check if the client is still
    /// there.
    Ping,
    /// Server-Sent-Event token to request a page update.
    Update,
}

pub fn manage_connections(
    event_tx_list: Arc<Mutex<Vec<SyncSender<SseToken>>>>,
    listener: TcpListener,
    doc_path: PathBuf,
) {
    // A list of referenced local links to images or other documents as
    // they appeared in the displayed documents.
    // Every thread gets an (ARC) reference to it.
    let allowed_urls = Arc::new(RwLock::new(HashSet::new()));
    // Subset of the above list containing only displayed Tp-Note documents.
    let delivered_tpnote_docs = Arc::new(RwLock::new(HashSet::new()));
    // We use an ARC to count the number of running threads.
    let conn_counter = Arc::new(());
    // Store `doc_path` in the `context.path` and
    // in the Tera variable `TMPL_VAR_PATH`.
    let context = Context::from(&doc_path);

    log::info!(
        "Viewer listens to incomming requests.\n\
        Besides all Tp-Note document extensions, \
        the following file extensions are served:\n\
        {}",
        {
            use std::fmt::Write;
            let mut list =
                CFG.viewer
                    .served_mime_types
                    .iter()
                    .fold(String::new(), |mut output, (k, _v)| {
                        let _ = write!(output, "{k}, ");
                        output
                    });
            list.truncate(list.len().saturating_sub(2));
            list
        }
    );

    for stream in listener.incoming() {
        match stream {
            Ok(stream) => {
                let (event_tx, event_rx) = sync_channel(0);
                event_tx_list.lock().unwrap().push(event_tx);
                thread::spawn({
                    let allowed_urls = allowed_urls.clone();
                    let delivered_tpnote_docs = delivered_tpnote_docs.clone();
                    let conn_counter = conn_counter.clone();
                    let context = context.clone();

                    move || {
                        let mut st = ServerThread::new(
                            event_rx,
                            stream,
                            allowed_urls,
                            delivered_tpnote_docs,
                            conn_counter,
                            context,
                        );
                        st.serve_connection()
                    }
                });
            }
            Err(e) => log::warn!("TCP connection failed: {}", e),
        }
    }
}

/// Server thread state.
pub(crate) struct ServerThread {
    /// Receiver side of the channel where `update` events are sent.
    rx: Receiver<SseToken>,
    /// Byte stream coming from a TCP connection.
    pub(crate) stream: TcpStream,
    /// A list of referenced relative URLs to images or other
    /// documents as they appear in the delivered Tp-Note documents.
    /// This list contains local links that may or may not have been displayed.
    /// The local links in this list are relative to `self.context.root_path`
    pub(crate) allowed_urls: Arc<RwLock<HashSet<PathBuf>>>,
    /// Subset of `allowed_urls` containing only URLs that
    /// have been actually delivered. The list only contains URLs to Tp-Note
    /// documents.
    /// The local links in this list are absolute.
    pub(crate) delivered_tpnote_docs: Arc<RwLock<HashSet<PathBuf>>>,
    /// We do not store anything here, instead we use the ARC pointing to
    /// `conn_counter` to count the number of instances of `ServerThread`.
    pub(crate) conn_counter: Arc<()>,
    /// The constructor stores the path of the note document in `context.path`
    /// and in the Tera variable `TMPL_VAR_PATH`.
    /// Both are needed for rendering to HTML.
    pub(crate) context: Context,
}

impl ServerThread {
    /// Constructor.
    fn new(
        rx: Receiver<SseToken>,
        stream: TcpStream,
        allowed_urls: Arc<RwLock<HashSet<PathBuf>>>,
        delivered_tpnote_docs: Arc<RwLock<HashSet<PathBuf>>>,
        conn_counter: Arc<()>,
        mut context: Context,
    ) -> Self {
        let local_addr = stream.local_addr();

        // Compose JavaScript code.
        let note_js = match local_addr {
            Ok(addr) => format!(
                "{}{}:{}{}",
                SSE_CLIENT_CODE1,
                LOCALHOST,
                addr.port(),
                SSE_CLIENT_CODE2
            ),
            Err(_) => {
                panic!("No TCP connection: socket address of local half is missing.")
            }
        };

        // Save JavaScript code.
        context.insert(TMPL_HTML_VAR_VIEWER_DOC_JS, &note_js);

        Self {
            rx,
            stream,
            allowed_urls,
            delivered_tpnote_docs,
            conn_counter,
            context,
        }
    }

    /// Wrapper for `serve_connection2()` that logs
    /// errors as log message warnings.
    fn serve_connection(&mut self) {
        match Self::serve_connection2(self) {
            Ok(_) => (),
            Err(e) => {
                log::debug!(
                    "TCP port local {} to peer {}: Closed connection because of error: {}",
                    self.stream
                        .local_addr()
                        .unwrap_or_else(|_| SocketAddr::V4(SocketAddrV4::new(
                            Ipv4Addr::new(0, 0, 0, 0),
                            0
                        )))
                        .port(),
                    self.stream
                        .peer_addr()
                        .unwrap_or_else(|_| SocketAddr::V4(SocketAddrV4::new(
                            Ipv4Addr::new(0, 0, 0, 0),
                            0
                        )))
                        .port(),
                    e
                );
            }
        }
    }

    /// HTTP server: serves content and events via the specified subscriber stream.
    #[inline]
    #[allow(clippy::needless_return)]
    fn serve_connection2(&mut self) -> Result<(), ViewerError> {
        // One reference is hold by the `manage_connections` thread and does not count.
        // This is why we subtract 1.
        let open_connections = Arc::<()>::strong_count(&self.conn_counter) - 1;
        log::trace!(
            "TCP port local {} to peer {}: New incoming TCP connection ({} open).",
            self.stream.local_addr()?.port(),
            self.stream.peer_addr()?.port(),
            open_connections
        );

        // Check if we exceed our connection limit.
        if open_connections > CFG.viewer.tcp_connections_max {
            self.respond_service_unavailable()?;
            // This ends this thread and closes the connection.
            return Err(ViewerError::TcpConnectionsExceeded {
                max_conn: CFG.viewer.tcp_connections_max,
            });
        }

        'tcp_connection: loop {
            // This is inspired by the Spook crate.
            // Read the request.
            let mut read_buffer = [0u8; TCP_READ_BUFFER_SIZE];
            let mut buffer = Vec::new();
            let (method, path) = 'assemble_tcp_chunks: loop {
                // Read the request, or part thereof.
                match self.stream.read(&mut read_buffer) {
                    Ok(0) => {
                        log::trace!(
                            "TCP port local {} to peer {}: Connection closed by peer.",
                            self.stream.local_addr()?.port(),
                            self.stream.peer_addr()?.port()
                        );
                        // Connection by peer.
                        break 'tcp_connection;
                    }
                    Err(e) => {
                        // Connection closed or error.
                        return Err(ViewerError::StreamRead { error: e });
                    }
                    Ok(n) => {
                        // Successful read.
                        buffer.extend_from_slice(&read_buffer[..n]);
                        log::trace!(
                            "TCP port local {} to peer {}: chunk: {:?} ...",
                            self.stream.local_addr()?.port(),
                            self.stream.peer_addr()?.port(),
                            std::str::from_utf8(&read_buffer)
                                .unwrap_or_default()
                                .chars()
                                .take(60)
                                .collect::<String>()
                        );
                    }
                }

                // Try to parse the request.
                let mut headers = [httparse::EMPTY_HEADER; 16];
                let mut req = httparse::Request::new(&mut headers);
                let res = req.parse(&buffer)?;
                if res.is_partial() {
                    continue 'assemble_tcp_chunks;
                }

                // Check if the HTTP header is complete and valid.
                if res.is_complete() {
                    if let (Some(method), Some(path)) = (req.method, req.path) {
                        // This is the only regular exit.
                        break 'assemble_tcp_chunks (method, path);
                    }
                };
                // We quit with error. There is nothing more we can do here.
                return Err(ViewerError::StreamParse {
                    source_str: std::str::from_utf8(&buffer)
                        .unwrap_or_default()
                        .chars()
                        .take(60)
                        .collect::<String>(),
                });
            };
            // End of input chunk loop.

            // The only supported request method for SSE is GET.
            if method != "GET" {
                self.respond_method_not_allowed(method)?;
                continue 'tcp_connection;
            }

            // Decode the percent encoding in the URL path.
            let path = percent_decode_str(path).decode_utf8()?;

            // Check the path.
            // Serve note rendition.
            match &*path {
                // This is a connection for Server-Sent-Events.
                SSE_EVENT_PATH => {
                    // Serve event response, but keep the connection.
                    self.respond_event_ok()?;
                    // Make the stream non-blocking to be able to detect whether the
                    // connection was closed by the client.
                    self.stream.set_nonblocking(true)?;

                    // Serve events until the connection is closed.
                    // Keep in mind that the client will often close
                    // the request after the first event if the event
                    // is used to trigger a page refresh, so try to eagerly
                    // detect closed connections.
                    '_event: loop {
                        // Wait for the next update.
                        let msg = self.rx.recv()?;

                        // Detect whether the connection was closed.
                        match self.stream.read(&mut read_buffer) {
                            // Connection closed.
                            Ok(0) => {
                                log::trace!(
                                    "TCP port local {} to peer {}: Event connection closed by peer.",
                                    self.stream.local_addr()?.port(),
                                    self.stream.peer_addr()?.port()
                                );
                                // Our peer closed this connection, we finish also then.
                                break 'tcp_connection;
                            }
                            // Connection alive.
                            Ok(_) => {}
                            // `WouldBlock` is OK, all others not.
                            Err(e) => {
                                if e.kind() != ErrorKind::WouldBlock {
                                    // Something bad happened.
                                    return Err(ViewerError::StreamRead { error: e });
                                }
                            }
                        }

                        // Send event.
                        let event = match msg {
                            SseToken::Update => "event: update\r\ndata:\r\n\r\n".to_string(),
                            SseToken::Ping => ": ping\r\n\r\n".to_string(),
                        };
                        self.stream.write_all(event.as_bytes())?;
                        log::trace!(
                            "TCP port local {} to peer {} ({} open TCP conn.): pushed '{:?}' in event connection to web browser.",
                            self.stream.local_addr()?.port(),
                            self.stream.peer_addr()?.port(),
                            Arc::<()>::strong_count(&self.conn_counter) - 1,
                            msg,
                        );
                    }
                }

                // Serve all other documents.
                _ => self.respond(&path)?,
            }; // end of match path
        } // Go to 'tcp_connection loop start

        log::trace!(
            "TCP port local {} to peer {}: ({} open). Closing this TCP connection.",
            self.stream.local_addr()?.port(),
            self.stream.peer_addr()?.port(),
            // We subtract 1 for the `manage connection()` thread, and
            // 1 for the thread we will close in a moment.
            Arc::<()>::strong_count(&self.conn_counter) - 2,
        );
        // We came here because the client closed this connection.
        Ok(())
    }

    /// Write HTTP event response.
    fn respond_event_ok(&mut self) -> Result<(), ViewerError> {
        // Declare SSE capability and allow cross-origin access.
        let response = format!(
            "\
             HTTP/1.1 200 OK\r\n\
             Date: {}\r\n\
             Access-Control-Allow-Origin: *\r\n\
             Cache-Control: no-cache\r\n\
             Content-Type: text/event-stream\r\n\
             \r\n",
            httpdate::fmt_http_date(SystemTime::now()),
        );
        self.stream.write_all(response.as_bytes())?;

        log::debug!(
            "TCP port local {} to peer {}: 200 OK, served event header, \
            keeping event connection open ...",
            self.stream.local_addr()?.port(),
            self.stream.peer_addr()?.port(),
        );
        Ok(())
    }
}
