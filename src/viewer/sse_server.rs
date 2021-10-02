//! Server-sent-event server for the note viewer feature.
//! This module contains also the web browser Javascript client code.

extern crate parse_hyperlinks;
extern crate percent_encoding;
extern crate tera;
extern crate url;

use crate::config::CFG;
use crate::config::VIEWER_SERVED_MIME_TYPES_HMAP;
use crate::note::Note;
use crate::viewer::error::ViewerError;
use crate::viewer::init::LOCALHOST;
use parse_hyperlinks_extras::iterator_html::HyperlinkInlineImage;
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
use url::Url;

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

/// String alias that can be used in paths instead of `../` which is ignored by web
/// browsers in leading position.
const PATH_UPDIR_ALIAS: &str = "ParentDir..";

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
    // `unwrap()` is Ok here here, because we just did it before successfully.
    let sse_port = listener.local_addr().unwrap().port();
    // A list of in the not referenced local links to images or other documents.
    // Every thread gets an (ARC) reference to it.
    let doc_local_links = Arc::new(RwLock::new(HashSet::new()));
    // We use an ARC to count the number of running threads.
    let conn_counter = Arc::new(());

    for stream in listener.incoming() {
        match stream {
            Ok(stream) => {
                let (event_tx, event_rx) = sync_channel(0);
                event_tx_list.lock().unwrap().push(event_tx);
                let doc_path = doc_path.clone();
                let doc_local_links = doc_local_links.clone();
                let conn_counter = conn_counter.clone();
                thread::spawn(move || {
                    let mut st = ServerThread::new(
                        event_rx,
                        stream,
                        sse_port,
                        doc_path,
                        doc_local_links,
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
    /// The TCP port this stream comes from.
    sse_port: u16,
    /// Local file system path of the note document.
    doc_path: PathBuf,
    /// A list of in the not referenced local links to images or other
    /// documents.
    doc_local_links: Arc<RwLock<HashSet<PathBuf>>>,
    /// We do not store anything here, instead we use the ARC pointing to
    /// `conn_counter` to count the number of instances of `ServerThread`.
    conn_counter: Arc<()>,
}

impl ServerThread {
    /// Constructor.
    fn new(
        rx: Receiver<SseToken>,
        stream: TcpStream,
        sse_port: u16,
        doc_path: PathBuf,
        doc_local_links: Arc<RwLock<HashSet<PathBuf>>>,
        conn_counter: Arc<()>,
    ) -> Self {
        Self {
            rx,
            stream,
            sse_port,
            doc_path,
            doc_local_links,
            conn_counter,
        }
    }

    /// Wrapper for `serve_connection2()` that logs
    /// errors as log message warnings.
    fn serve_connection(&mut self) {
        match Self::serve_connection2(self) {
            Ok(_) => (),
            Err(e) => {
                log::debug!(
                    "TCP peer port {}: Closed connection because of error: {}",
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
            "TCP peer port {}: New incoming TCP connection ({} open).",
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
                            "TCP peer port {}: Connection closed by peer.",
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
                            "TCP peer port {}: chunk: {:?} ...",
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
                    // The contains Javascript code to subscribe to `EVENT_PATH`, which
                    // reloads this document on request of `self.rx`.
                    let html = Self::render_content_and_error(self)?;

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
                                    "TCP peer port {}: Event connection closed by peer.",
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
                            "TCP peer port {} ({} open TCP conn.): pushed '{:?}' in event connection to web browser.",
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
                    // Concatenate document directory and URL path.
                    let doc_path = self.doc_path.canonicalize()?;
                    #[allow(clippy::or_fun_call)]
                    let doc_dir = doc_path.parent().unwrap_or(Path::new(""));

                    // Strip `/` and convert to `Path`.
                    let path = Path::new(
                        path.strip_prefix('/')
                            .ok_or(ViewerError::UrlMustStartWithSlash)?,
                    );
                    // Replace `PATH_UPDIR_ALIAS` with `..`.
                    let mut reqpath = doc_dir.to_owned();
                    for p in path.iter() {
                        if p == "." {
                            continue;
                        }
                        if p == PATH_UPDIR_ALIAS || p == ".." {
                            reqpath.pop();
                        } else {
                            reqpath.push(p);
                        }
                    }

                    // Condition 1.: Check if we serve this kind of extension
                    let extension = &*reqpath
                        .extension()
                        .unwrap_or_default()
                        .to_str()
                        .unwrap_or_default()
                        .to_lowercase();
                    // Find the corresponding mime type of this file extension.
                    let mime_type = match VIEWER_SERVED_MIME_TYPES_HMAP.get(&*extension) {
                        Some(mt) => mt,
                        None => {
                            // Reject all files with extensions not listed.
                            log::warn!(
                                "TCP peer port {}: \
                                files with extension '{}' are not served. Rejecting: '{}'",
                                self.stream.peer_addr()?.port(),
                                reqpath
                                    .extension()
                                    .unwrap_or_default()
                                    .to_str()
                                    .unwrap_or_default(),
                                reqpath.to_str().unwrap_or_default(),
                            );
                            self.respond_not_found(&reqpath)?;
                            continue 'tcp_connection;
                        }
                    };

                    // Condition 2.: Only serve files that explicitly appear in
                    // `self.doc_local_links`.
                    let doc_local_links = self
                        .doc_local_links
                        .read()
                        .expect("Can not read `doc_local_links`! RwLock is poisoned. Panic.");

                    if !doc_local_links.contains(path) {
                        log::warn!(
                            "TCP peer port {}: target not referenced in note file, rejecting: '{}'",
                            self.stream.peer_addr()?.port(),
                            path.to_str().unwrap_or(""),
                        );
                        drop(doc_local_links);
                        self.respond_not_found(&reqpath)?;
                        continue 'tcp_connection;
                    }
                    // Release the `RwLockReadGuard`.
                    drop(doc_local_links);

                    // Condition 3.: Only serve resources in the same or under the document's
                    // parent directory.
                    #[allow(clippy::or_fun_call)]
                    let doc_parent_dir = doc_dir.parent().unwrap_or(Path::new(""));
                    if !reqpath.starts_with(doc_parent_dir) {
                        log::warn!(
                            "TCP peer port {}:\
                                file '{}' is not in directory '{}', rejecting.",
                            self.stream.peer_addr()?.port(),
                            reqpath.to_str().unwrap_or_default(),
                            doc_parent_dir.to_str().unwrap_or_default()
                        );
                        self.respond_not_found(&reqpath)?;
                        continue 'tcp_connection;
                    }

                    // Condition 4.: Is the file readable?
                    if fs::metadata(&reqpath)?.is_file() {
                        self.respond_file_ok(&reqpath, mime_type)?;
                    } else {
                        self.respond_not_found(&reqpath)?;
                    }
                }
            }; // end of match path
        } // Go to 'tcp_connection loop start

        log::trace!(
            "TCP peer port {}: ({} open). Closing this TCP connection.",
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
            "TCP peer port {}: 200 OK, served event header, \
            keeping event connection open ...",
            self.stream.peer_addr()?.port(),
        );
        Ok(())
    }

    /// Write HTTP OK response with content.
    fn respond_file_ok(&mut self, reqpath: &Path, mime_type: &str) -> Result<(), ViewerError> {
        let response = format!(
            "HTTP/1.1 200 OK\r\n\
             Date: {}\r\n\
             Cache-Control: private, max-age={}\r\n\
             Content-Type: {}\r\n\
             Content-Length: {}\r\n\r\n",
            httpdate::fmt_http_date(SystemTime::now()),
            MAX_AGE.to_string(),
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
            "TCP peer port {}: 200 OK, served file: '{}'",
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
            MAX_AGE.to_string(),
            mime_type,
            content.len(),
        );
        self.stream.write_all(response.as_bytes())?;
        self.stream.write_all(content)?;
        log::debug!(
            "TCP peer port {}: 200 OK, served file: '{}'",
            self.stream.peer_addr()?.port(),
            reqpath.to_str().unwrap_or_default()
        );

        Ok(())
    }

    /// Write HTTP not found response.
    fn respond_not_found(&mut self, reqpath: &Path) -> Result<(), ViewerError> {
        self.stream.write_all(b"HTTP/1.1 404 Not Found\r\n\r\n")?;
        log::debug!(
            "TCP peer port {}: 404 \"Not found\" served: '{}'",
            self.stream.peer_addr()?.port(),
            reqpath.to_str().unwrap_or_default()
        );
        Ok(())
    }

    /// Write HTTP not found response.
    fn respond_method_not_allowed(&mut self, path: &str) -> Result<(), ViewerError> {
        self.stream
            .write_all(b"HTTP/1.1 405 Method Not Allowed\r\n\r\n")?;
        log::debug!(
            "TCP peer port {}: 405 \"Method Not Allowed\" served: '{}'",
            self.stream.peer_addr()?.port(),
            path,
        );
        Ok(())
    }

    /// Write HTTP service unavailable response.
    fn respond_service_unavailable(&mut self) -> Result<(), ViewerError> {
        self.stream
            .write_all(b"HTTP/1.1 503 Service Unavailable\r\n\r\n")?;
        Ok(())
    }

    #[inline]
    /// Renders the error page with the `VIEWER_ERROR_TMPL`.
    fn render_content_and_error(&self) -> Result<String, ViewerError> {
        // Deserialize.
        let js = format!(
            "{}{}:{}{}",
            SSE_CLIENT_CODE1, LOCALHOST, self.sse_port, SSE_CLIENT_CODE2
        );

        // Extension determines markup language when rendering.
        let file_path_ext = self
            .doc_path
            .extension()
            .unwrap_or_default()
            .to_str()
            .unwrap_or_default();

        // Render.
        // First decompose header and body, then deserialize header.
        match Note::from_existing_note(&self.doc_path)
            // Now, try to render to html.
            .and_then(|mut note| {
                note.render_content(file_path_ext, &CFG.viewer.rendition_tmpl, &js)
            })
            // Now scan the HTML result for links and store them in a HashMap
            // accessible to all threads.
            .and_then(|html| {
                let mut doc_local_links = self
                    .doc_local_links
                    .write()
                    .expect("Can not write `doc_local_links`. RwLock is poisoned. Panic.");

                // Populate the list from scratch.
                doc_local_links.clear();

                // Search for hyperlinks and inline images in the HTML rendition
                // of this note.
                for ((_, _, _), link) in HyperlinkInlineImage::new(&html) {
                    // We skip absolute URLs.
                    if let Ok(url) = Url::parse(&link) {
                        if url.has_host() {
                            continue;
                        };
                    };
                    let path = PathBuf::from(&*percent_decode_str(&link).decode_utf8()?);
                    // Save the hyperlinks for other threads to check against.
                    doc_local_links.insert(path);
                }

                if doc_local_links.is_empty() {
                    log::debug!(
                        "Viewer: note file has no local hyperlinks. No additional local files are served.",
                    );
                } else {
                    log::info!(
                        "Viewer: referenced local files: {}",
                        doc_local_links
                        .iter()
                        .map(|p|{
                            let mut s = "\n    '".to_string();
                            s.push_str(p.as_path().to_str().unwrap_or_default());
                            s
                        }).collect::<String>()
                    );
                }
                Ok(html)
                // The `RwLockWriteGuard` is released here.
            }) {
            // If the rendition went well, return the HTML.
            Ok(html) => Ok(html),
            // We could not render the note properly. Instead we will render a
            // special error page and return this instead.
            Err(e) => {
                // Render error page providing all information we have.
                Note::render_erroneous_content(&self.doc_path, &CFG.viewer.error_tmpl, &js, e)
                    .map_err(|e| { ViewerError::RenderErrorPage {
                        tmpl: "[viewer] error_tmpl".to_string(),
                        source: e,
                    }})
            }
        }
    }
}
