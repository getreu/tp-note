//! Server-sent-event server for the note viewer feature.
//! This module contains also the web browser JavaScript client code.

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
use std::sync::mpsc::{Receiver, SyncSender, sync_channel};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::SystemTime;
use tpnote_lib::context::{Context, HasSettings};

/// The TCP stream is read in chunks. This is the read buffer size.
const TCP_READ_BUFFER_SIZE: usize = 0x400;

/// JavaScript client code, part 1
/// Refresh on `WTFiles` events.
pub const SSE_CLIENT_CODE1: &str = r#"
    var evtSource = new EventSource("http://"#;
/// JavaScript client code, part 2
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

/// Checks whether an HTTP `Host` header value addresses this server on the
/// loopback interface. `local_port` is the port the listener is bound to; a
/// port in the header value, if present, must match it. A missing `Host`
/// header (`None`, legal in HTTP/1.0) is allowed: such clients (e.g. plain
/// `curl -0`) are local tools, not browsers, and the DNS rebinding attack
/// this check defends against always sends the hostile origin as `Host`.
fn host_is_local(host: Option<&str>, local_port: u16) -> bool {
    let Some(host) = host else {
        return true;
    };
    let host = host.trim();
    // Split off the optional `:<port>`. A bracketed IPv6 literal contains
    // colons itself, so it needs its own splitting rule.
    let (name, port) = if host.starts_with('[') {
        match host.find(']') {
            Some(i) => match host[i + 1..].strip_prefix(':') {
                Some(p) => (&host[..=i], Some(p)),
                None if host[i + 1..].is_empty() => (host, None),
                None => return false,
            },
            None => return false,
        }
    } else if let Some((name, port)) = host.rsplit_once(':') {
        (name, Some(port))
    } else {
        (host, None)
    };
    if !(name.eq_ignore_ascii_case("localhost") || name == "127.0.0.1" || name == "[::1]") {
        return false;
    }
    match port {
        None => true,
        Some(p) => p.parse::<u16>().is_ok_and(|p| p == local_port),
    }
}

/// Server-Sent-Event tokens our HTTP client has registered to receive.
#[derive(Debug, Clone, Copy)]
pub enum SseToken {
    /// Server-Sent-Event token to request nothing but check if the client is
    /// still there.
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
    let context = Context::from(&doc_path).expect("can not access document path");
    //

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
    pub(crate) context: Context<HasSettings>,
    /// Java Script injection code used by the root page for live updates.
    /// Root pages insert this in their context with the key
    /// `TMPL_HTML_VAR_VIEWR_DOC_JS`.
    pub(crate) live_update_js: String,
    /// Set only on the request that binds the session
    /// (`viewer.session_binding`); cleared by `respond_content_ok()` after
    /// the `Set-Cookie` header was successfully written. `Some` therefore
    /// means: bound, but the cookie was not yet delivered to the client —
    /// the condition the binding rollback in `serve_connection2()` tests.
    pub(crate) set_cookie: Option<String>,
}

impl ServerThread {
    /// Constructor.
    fn new(
        rx: Receiver<SseToken>,
        stream: TcpStream,
        allowed_urls: Arc<RwLock<HashSet<PathBuf>>>,
        delivered_tpnote_docs: Arc<RwLock<HashSet<PathBuf>>>,
        conn_counter: Arc<()>,
        context: Context<HasSettings>,
    ) -> Self {
        let local_addr = stream.local_addr();

        // Compose JavaScript code.
        let live_update_js = match local_addr {
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

        Self {
            rx,
            stream,
            allowed_urls,
            delivered_tpnote_docs,
            conn_counter,
            context,
            live_update_js,
            set_cookie: None,
        }
    }

    /// Formats the `Set-Cookie` header line for the response that binds the
    /// session; the empty string on every other response.
    /// `HttpOnly`: JS never needs the cookie. `SameSite=Lax`: blocks the
    /// cookie on cross-site non-navigation requests (a hostile page's
    /// `fetch`/`<img>`/`<iframe>`) while keeping legitimate top-level
    /// navigation to `localhost` working. `Path=/`: covers `/events`,
    /// images and linked notes. Host-only session cookie: no `Domain`, no
    /// `Expires`, no `Secure` (plain http on localhost).
    pub(crate) fn set_cookie_header(&self) -> String {
        match self.set_cookie.as_deref() {
            Some(tok) => {
                format!("Set-Cookie: tpnote={tok}; HttpOnly; SameSite=Lax; Path=/\r\n")
            }
            None => String::new(),
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
            let (method, path, host) = 'assemble_tcp_chunks: loop {
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
                let mut headers = [httparse::EMPTY_HEADER; 32];
                let mut req = httparse::Request::new(&mut headers);
                let res = req.parse(&buffer)?;
                if res.is_partial() {
                    continue 'assemble_tcp_chunks;
                }

                // Check if the HTTP header is complete and valid.
                if res.is_complete()
                    && let (Some(method), Some(path)) = (req.method, req.path) {
                        // Extract headers as owned values before `req` (and
                        // its borrow of `headers`) goes out of scope.
                        let host: Option<String> = req
                            .headers
                            .iter()
                            .find(|h| h.name.eq_ignore_ascii_case("Host"))
                            .and_then(|h| str::from_utf8(h.value).ok())
                            .map(|s| s.trim().to_string());
                        // This is the only regular exit.
                        break 'assemble_tcp_chunks (method, path, host);
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

            // Refuse requests with a foreign `Host` header. Kills DNS
            // rebinding (a hostile origin re-resolving to 127.0.0.1 issues
            // same-origin, i.e. readable, requests — but with its own
            // `Host`). No legitimate client ever sends a foreign `Host` to
            // this server, so this check applies unconditionally.
            if !host_is_local(host.as_deref(), self.stream.local_addr()?.port()) {
                self.respond_forbidden()?;
                // Refuse the request AND close this connection.
                return Err(ViewerError::SessionCookieMismatch);
            }

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
            }; // End of match path
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

#[cfg(test)]
mod tests {
    use super::host_is_local;

    #[test]
    fn test_host_is_local() {
        // Bare local names, with and without the matching port.
        assert!(host_is_local(Some("localhost"), 4444));
        assert!(host_is_local(Some("localhost:4444"), 4444));
        assert!(host_is_local(Some("Localhost:4444"), 4444));
        assert!(host_is_local(Some("127.0.0.1"), 4444));
        assert!(host_is_local(Some("127.0.0.1:4444"), 4444));
        assert!(host_is_local(Some("[::1]"), 4444));
        assert!(host_is_local(Some("[::1]:4444"), 4444));
        // HTTP/1.0 clients may omit the header: allowed (documented choice).
        assert!(host_is_local(None, 4444));

        // Foreign hosts and wrong ports are refused.
        assert!(!host_is_local(Some("evil.example"), 4444));
        assert!(!host_is_local(Some("evil.example:4444"), 4444));
        assert!(!host_is_local(Some("localhost:3333"), 4444));
        assert!(!host_is_local(Some("localhost:x"), 4444));
        assert!(!host_is_local(Some("[::1]:3333"), 4444));
        assert!(!host_is_local(Some("[::2]:4444"), 4444));
        assert!(!host_is_local(Some("[::1"), 4444));
        assert!(!host_is_local(Some("[::1]4444"), 4444));
        assert!(!host_is_local(Some(""), 4444));
    }
}
