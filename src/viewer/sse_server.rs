//! Server-sent-event server for the note viewer feature.
//! This module contains also the web browser Javascript client code.

use crate::config::CFG;
use crate::config::VIEWER_SERVED_MIME_TYPES_HMAP;
use crate::viewer::error::ViewerError;
use crate::viewer::init::LOCALHOST;
use percent_encoding::percent_decode_str;
use std::collections::HashSet;
use std::fs;
use std::io::{ErrorKind, Read, Write};
use std::net::Ipv4Addr;
use std::net::SocketAddr;
use std::net::SocketAddrV4;
use std::net::{TcpListener, TcpStream};
use std::path::Path;
use std::path::PathBuf;
use std::str;
use std::sync::mpsc::{sync_channel, Receiver, SyncSender};
use std::sync::{Arc, Mutex, RwLock};
use std::thread;
use std::time::SystemTime;
use tpnote_lib::config::TMPL_VAR_NOTE_ERROR;
use tpnote_lib::config::TMPL_VAR_NOTE_JS;
use tpnote_lib::content::Content;
use tpnote_lib::content::ContentString;
use tpnote_lib::context::Context;
use tpnote_lib::html::rewrite_links;
use tpnote_lib::markup_language::MarkupLanguage;
use tpnote_lib::workflow::render_erroneous_content_html;
use tpnote_lib::workflow::render_viewer_html;

/// The TCP stream is read in chunks. This is the read buffer size.
const TCP_READ_BUFFER_SIZE: usize = 0x400;

/// Content from files are served in chunks.
const TCP_WRITE_BUFFER_SIZE: usize = 0x1000;

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

/// Modern browser request a small icon image.
pub const FAVICON: &[u8] = include_bytes!("favicon.ico");
/// The path where the favicon is requested.
pub const FAVICON_PATH: &str = "/favicon.ico";

/// Time in seconds the browsers should keep the delivered content in cache.
const MAX_AGE: u64 = 600;

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

    for stream in listener.incoming() {
        match stream {
            Ok(stream) => {
                let (event_tx, event_rx) = sync_channel(0);
                event_tx_list.lock().unwrap().push(event_tx);
                let doc_path = doc_path.clone();
                let allowed_urls = allowed_urls.clone();
                let delivered_tpnote_docs = delivered_tpnote_docs.clone();
                let conn_counter = conn_counter.clone();
                thread::spawn(move || {
                    let mut st = ServerThread::new(
                        event_rx,
                        stream,
                        doc_path,
                        allowed_urls,
                        delivered_tpnote_docs,
                        conn_counter,
                    );
                    st.serve_connection()
                });
            }
            Err(e) => log::warn!("TCP connection failed: {}", e),
        }
    }
}

/// Server thread state.
struct ServerThread {
    /// Receiver side of the channel where `update` events are sent.
    rx: Receiver<SseToken>,
    /// Byte stream coming from a TCP connection.
    stream: TcpStream,
    /// A list of referenced relative URLs to images or other
    /// documents as they appear in the delivered Tp-Note documents.
    /// This list contains URLs that may or may not have been displayed.
    allowed_urls: Arc<RwLock<HashSet<PathBuf>>>,
    /// Subset of `allowed_urls` containing only URLs that
    /// have been actually delivered. The list only contains URLs to Tp-Note
    /// documents.
    delivered_tpnote_docs: Arc<RwLock<HashSet<PathBuf>>>,
    /// We do not store anything here, instead we use the ARC pointing to
    /// `conn_counter` to count the number of instances of `ServerThread`.
    conn_counter: Arc<()>,
    /// The constructor stores the path of the note document in `context.path`
    /// and in the Tera variable `TMPL_VAR_PATH`.
    /// Both are needed for rendering to HTML.
    context: Context,
}

impl ServerThread {
    /// Constructor.
    fn new(
        rx: Receiver<SseToken>,
        stream: TcpStream,
        doc_path: PathBuf,
        allowed_urls: Arc<RwLock<HashSet<PathBuf>>>,
        delivered_tpnote_docs: Arc<RwLock<HashSet<PathBuf>>>,
        conn_counter: Arc<()>,
    ) -> Self {
        // Store `doc_path` in the `context.path` and
        // in the Tera variable `TMPL_VAR_PATH`.
        let mut context = Context::from(&doc_path);
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
        context.insert(TMPL_VAR_NOTE_JS, &note_js);

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
                    source_str: std::str::from_utf8(&*buffer)
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
                // The client wants the rendered note.
                "/" => {
                    // Renders a content page or an error page for the current note.
                    // Tera template errors.
                    // The contains JavaScript code to subscribe to `EVENT_PATH`, which
                    // reloads this document on request of `self.rx`.
                    let html = self.render_content_and_error(&self.context.path)?;

                    self.respond_content_ok(Path::new("/"), "text/html", html.as_bytes())?;
                    // `self.rx` was not used and is dropped here.
                }

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
                        log::debug!(
                            "TCP port local {} to peer {} ({} open TCP conn.): pushed '{:?}' in event connection to web browser.",
                            self.stream.local_addr()?.port(),
                            self.stream.peer_addr()?.port(),
                            Arc::<()>::strong_count(&self.conn_counter) - 1,
                            msg,
                        );
                    }
                }

                // Serve icon.
                FAVICON_PATH => {
                    self.respond_content_ok(Path::new(&FAVICON_PATH), "image/x-icon", FAVICON)?;
                }

                // Serve all other documents.
                _ => {
                    // Assert starting with `/`.
                    let abspath = Path::new(path.as_ref());
                    if !abspath.is_absolute() {
                        return Err(ViewerError::UrlMustStartWithSlash);
                    }

                    // Condition 1: Check if we serve this kind of extension
                    let extension = &*abspath
                        .extension()
                        .unwrap_or_default()
                        .to_str()
                        .unwrap_or_default()
                        .to_lowercase();

                    // Find the corresponding mime type of this file extension.
                    let mime_type = match VIEWER_SERVED_MIME_TYPES_HMAP.get(extension) {
                        Some(mt) => mt,
                        None => {
                            // Reject all files with extensions not listed.
                            log::warn!(
                                "TCP port local {} to peer {}: \
                                files with extension '{}' are not served. Rejecting: '{}'",
                                self.stream.local_addr()?.port(),
                                self.stream.peer_addr()?.port(),
                                abspath
                                    .extension()
                                    .unwrap_or_default()
                                    .to_str()
                                    .unwrap_or_default(),
                                abspath.to_str().unwrap_or_default(),
                            );
                            self.respond_not_found(abspath)?;
                            continue 'tcp_connection;
                        }
                    };

                    // Condition 2: Only serve files that explicitly appear in
                    // `self.allowed_urls`.
                    let allowed_urls = self
                        .allowed_urls
                        .read()
                        .expect("Can not read `allowed_urls`! RwLock is poisoned. Panic.");

                    if !allowed_urls.contains(abspath) {
                        log::warn!(
                            "TCP port local {} to peer {}: target not referenced in note file, rejecting: '{}'",
                            self.stream.local_addr()?.port(),
                            self.stream.peer_addr()?.port(),
                            abspath.to_str().unwrap_or(""),
                        );
                        // Release the `RwLockReadGuard`.
                        drop(allowed_urls);
                        self.respond_not_found(abspath)?;
                        continue 'tcp_connection;
                    }

                    // Release the `RwLockReadGuard`.
                    drop(allowed_urls);

                    // Condition 3: Is the file a child of `root_path`?
                    if !abspath.starts_with(&self.context.root_path) {
                        self.respond_forbidden(abspath)?;
                        log::warn!(
                            "TCP port local {} to peer {}: target path does not start with '{}', rejecting: '{}'",
                            self.stream.local_addr()?.port(),
                            self.stream.peer_addr()?.port(),
                            self.context.root_path.to_str().unwrap_or(""),
                            abspath.to_str().unwrap_or(""),
                        );
                        continue 'tcp_connection;
                    }

                    // Condition 4: If this is a Tp-Note file, check the maximum
                    // of delivered documents.
                    if !matches!(extension.into(), MarkupLanguage::None) {
                        if abspath.is_file() {
                            let delivered_docs_count = self
                                .delivered_tpnote_docs
                                .read()
                                .expect("Can not read `delivered_tpnote_docs`. RwLock is poisoned. Panic.")
                                .len();
                            if delivered_docs_count < CFG.viewer.displayed_tpnote_count_max {
                                let html = self.render_content_and_error(abspath)?;
                                self.respond_content_ok(abspath, "text/html", html.as_bytes())?;
                            } else {
                                self.respond_too_many_requests()?;
                            }
                            continue 'tcp_connection;
                        } else {
                            log::info!(
                                "Referenced Tp-Note file not found: {}",
                                abspath.to_str().unwrap_or_default()
                            );
                            self.respond_not_found(abspath)?;
                            continue 'tcp_connection;
                        }
                    }

                    // Condition 5: Is the file readable?
                    if abspath.is_file() {
                        self.respond_file_ok(abspath, mime_type)?;
                    } else {
                        self.respond_not_found(abspath)?;
                    }
                }
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

    /// Write HTTP event response.
    // fn respond_no_content_ok(&mut self) -> Result<(), ViewerError> {
    //     self.stream
    //         .write_all(b"HTTP/1.1 204\r\nContent-Length: 0\r\n\r\n")?;
    //     log::debug!(
    //         "TCP port local {} to peer {}: 204 OK, served header, \
    //         keeping event connection open ...",
    //         self.stream.local_addr()?.port(),
    //         self.stream.peer_addr()?.port(),
    //     );
    //     Ok(())
    // }

    /// Write HTTP OK response with content.
    fn respond_file_ok(&mut self, reqpath: &Path, mime_type: &str) -> Result<(), ViewerError> {
        let response = format!(
            "HTTP/1.1 200 OK\r\n\
             Date: {}\r\n\
             Cache-Control: private, max-age={}\r\n\
             Content-Type: {}\r\n\
             Content-Length: {}\r\n\r\n",
            httpdate::fmt_http_date(SystemTime::now()),
            MAX_AGE,
            mime_type,
            fs::metadata(&reqpath)?.len(),
        );
        self.stream.write_all(response.as_bytes())?;

        // Serve file in chunks.
        let mut buffer = [0; TCP_WRITE_BUFFER_SIZE];
        let mut file = fs::File::open(&reqpath)?;

        while let Ok(n) = file.read(&mut buffer[..]) {
            if n == 0 {
                break;
            };
            self.stream.write_all(&buffer[..n])?;
        }

        log::debug!(
            "TCP port local {} to peer {}: 200 OK, served file: '{}'",
            self.stream.local_addr()?.port(),
            self.stream.peer_addr()?.port(),
            reqpath.to_str().unwrap_or_default()
        );

        Ok(())
    }

    /// Write HTTP OK response with content.
    fn respond_content_ok(
        &mut self,
        reqpath: &Path,
        mime_type: &str,
        content: &[u8],
    ) -> Result<(), ViewerError> {
        let response = format!(
            "HTTP/1.1 200 OK\r\n\
             Date: {}\r\n\
             Cache-Control: private, max-age={}\r\n\
             Content-Type: {}\r\n\
             Content-Length: {}\r\n\r\n",
            httpdate::fmt_http_date(SystemTime::now()),
            MAX_AGE,
            mime_type,
            content.len(),
        );
        self.stream.write_all(response.as_bytes())?;
        self.stream.write_all(content)?;
        log::debug!(
            "TCP port local {} to peer {}: 200 OK, served file: '{}'",
            self.stream.local_addr()?.port(),
            self.stream.peer_addr()?.port(),
            reqpath.to_str().unwrap_or_default()
        );

        Ok(())
    }

    /// Write HTTP not found response.
    fn respond_forbidden(&mut self, reqpath: &Path) -> Result<(), ViewerError> {
        self.stream
            .write_all(b"HTTP/1.1 403\r\nContent-Length: 0\r\n\r\n")?;
        log::debug!(
            "TCP port local {} to peer {}: 403 \"Forbidden\" request was: '{}'",
            self.stream.local_addr()?.port(),
            self.stream.peer_addr()?.port(),
            reqpath.to_str().unwrap_or_default()
        );
        Ok(())
    }

    /// Write HTTP not found response.
    fn respond_not_found(&mut self, reqpath: &Path) -> Result<(), ViewerError> {
        self.stream
            .write_all(b"HTTP/1.1 404\r\nContent-Length: 0\r\n\r\n")?;
        log::debug!(
            "TCP port local {} to peer {}: 404 \"Not found\" request was: '{}'",
            self.stream.local_addr()?.port(),
            self.stream.peer_addr()?.port(),
            reqpath.to_str().unwrap_or_default()
        );
        Ok(())
    }

    /// Write HTTP method not allowed response.
    fn respond_method_not_allowed(&mut self, path: &str) -> Result<(), ViewerError> {
        self.stream
            .write_all(b"HTTP/1.1 405\r\nContent-Length: 0\r\n\r\n")?;
        log::debug!(
            "TCP port local {} to peer {}: 405 \"Method Not Allowed\" request was: '{}'",
            self.stream.local_addr()?.port(),
            self.stream.peer_addr()?.port(),
            path,
        );
        Ok(())
    }

    /// Write HTTP event response.
    fn respond_too_many_requests(&mut self) -> Result<(), ViewerError> {
        let delivered_tpnote_docs = self
            .delivered_tpnote_docs
            .read()
            .expect("Can not read `delivered_tpnote_docs`! RwLock is poisoned. Panic.");

        // Prepare the log entry.
        let mut log_msg = format!(
            "Error: too many requests. You have exceeded \n\
            `[viewer] displayed_tpnote_count_max = {}` by browsing:\n",
            CFG.viewer.displayed_tpnote_count_max
        );
        for p in delivered_tpnote_docs.iter() {
            log_msg.push_str("- ");
            log_msg.push_str(p.to_str().unwrap_or_default());
            log_msg.push('\n');
        }
        log::warn!("{}", log_msg);

        // Prepare the HTML output.
        let content = format!(
            "<!DOCTYPE html><html><head><meta charset=\"UTF-8\"></head>
             <body><h2>Too many requests</h2>
             <p>For security reasons, Tp-Note's internal viewer only displays
             a limited number ({}) of Tp-Note files. This limit can be raised
             by setting the configuration file variable:</p>
            <p> <pre>[viewer] displayed_tpnote_count_max</pre></p>
             </body></html>
             ",
            CFG.viewer.displayed_tpnote_count_max
        );

        let response = format!(
            "HTTP/1.1 429\r\n\
             Date: {}\r\n\
             Cache-Control: private, max-age={}\r\n\
             Content-Type: text/html\r\n\
             Content-Length: {}\r\n\r\n",
            httpdate::fmt_http_date(SystemTime::now()),
            MAX_AGE,
            content.len(),
        );
        self.stream.write_all(response.as_bytes())?;
        self.stream.write_all(content.as_bytes())?;
        log::debug!(
            "TCP port local {} to peer {}: 429 \"Too many requests\"",
            self.stream.local_addr()?.port(),
            self.stream.peer_addr()?.port(),
        );

        Ok(())
    }

    /// Write HTTP service unavailable response.
    fn respond_service_unavailable(&mut self) -> Result<(), ViewerError> {
        self.stream
            .write_all(b"HTTP/1.1 503\r\nContent-Length: 0\r\n\r\n")?;
        Ok(())
    }

    /// Renders the error page with the `HTML_VIEWER_ERROR_TMPL`.
    /// `abspath` points to the document with markup that should be rendered to HTML.
    /// The function injects `self.context` before rendering the template.
    fn render_content_and_error(&self, abspath_doc: &Path) -> Result<String, ViewerError> {
        // First decompose header and body, then deserialize header.
        let content = ContentString::open(abspath_doc)?;
        let abspath_dir = abspath_doc.parent().unwrap_or_else(|| Path::new("/"));
        let root_path = &self.context.root_path;

        // Only the first base document is live updated.
        let mut context = self.context.clone();
        if context.path != abspath_doc {
            context.insert(TMPL_VAR_NOTE_JS, "");
        }
        match render_viewer_html::<ContentString>(context, content)
            // Now scan the HTML result for links and store them in a HashMap
            // accessible to all threads.
            // Secondly, convert all relative links to absolute links.
            .map(|html| {
                rewrite_links(
                    html,
                    root_path,
                    abspath_dir,
                    // Do convert rel. to abs. links.
                    true,
                    // Do not append `.html` to `.md` links.
                    false,
                    self.allowed_urls.clone(),
                )
            }) {
            // If the rendition went well, return the HTML.
            Ok(html) => {
                let mut delivered_tpnote_docs = self
                    .delivered_tpnote_docs
                    .write()
                    .expect("Can not write `delivered_tpnote_docs`. RwLock is poisoned. Panic.");
                delivered_tpnote_docs.insert(abspath_doc.to_owned());
                log::info!(
                    "Viewer: displayed Tp-Note documents: {}",
                    delivered_tpnote_docs
                        .iter()
                        .map(|p| {
                            let mut s = "\n    '".to_string();
                            s.push_str(p.as_path().to_str().unwrap_or_default());
                            s
                        })
                        .collect::<String>()
                );
                Ok(html)
            }
            // We could not render the note properly. Instead we will render a
            // special error page and return this instead.
            Err(e) => {
                // Render error page providing all information we havStringe.
                let mut context = self.context.clone();
                context.insert(TMPL_VAR_NOTE_ERROR, &e.to_string());
                let note_erroneous_content = <ContentString as Content>::open(&context.path)?;
                render_erroneous_content_html::<ContentString>(context, note_erroneous_content)
                    .map_err(|e| ViewerError::RenderErrorPage {
                        tmpl: "[tmpl_html] viewer_error".to_string(),
                        source: e,
                    })
            }
        }
    }
}
