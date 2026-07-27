//! Server-sent-event server for the note viewer feature.
//! This module contains also the web browser JavaScript client code.

use crate::config::CFG;
#[cfg(feature = "same-user-policy")]
use crate::config::SameUserPolicy;
use crate::viewer::error::ViewerError;
use crate::viewer::http_response::HttpResponse;
#[cfg(feature = "same-user-policy")]
use crate::viewer::http_response::{peer_user_mismatch_page, peer_user_unknown_page};
use crate::viewer::init::LOCALHOST;
#[cfg(feature = "same-user-policy")]
use crate::viewer::peer_user::{PeerUser, identify_peer_user};
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
use std::sync::{Arc, Condvar, Mutex};
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

/// Returns a fresh random session token: 16 bytes from the OS CSPRNG,
/// hex-encoded to exactly 32 characters.
fn new_session_token() -> String {
    let mut buf = [0u8; 16];
    getrandom::fill(&mut buf).expect("OS CSPRNG (getrandom) failed");
    let mut s = String::with_capacity(32);
    for b in buf {
        use std::fmt::Write;
        let _ = write!(s, "{:02x}", b);
    }
    s
}

/// Extracts the value of the `tpnote` cookie from an HTTP `Cookie` header
/// value like `a=1; tpnote=deadbeef; b=2`. Returns `None` if absent.
fn parse_tpnote_cookie(header: &str) -> Option<String> {
    header
        .split(';')
        .find_map(|pair| pair.trim().strip_prefix("tpnote="))
        .map(str::to_string)
}

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
    start_accepting: Arc<(Mutex<bool>, Condvar)>,
    doc_path: PathBuf,
) {
    // A list of referenced local links to images or other documents as
    // they appeared in the displayed documents.
    // Every thread gets an (ARC) reference to it.
    let allowed_urls = Arc::new(RwLock::new(HashSet::new()));
    // Subset of the above list containing only displayed Tp-Note documents.
    let delivered_tpnote_docs = Arc::new(RwLock::new(HashSet::new()));
    // The session token the viewer is bound to, `None` while unbound
    // (`viewer.session_binding_cookie`). Process-wide: one binding per viewer.
    let session_cookie: Arc<RwLock<Option<String>>> = Arc::new(RwLock::new(None));
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

    // Do not accept connections before the caller has launched the web
    // browser: this shrinks the window in which a foreign local client
    // could claim the session binding (`viewer.session_binding_cookie`) to the
    // browser's cold-start latency. The listener is already bound, so
    // clients connecting early queue in the TCP backlog instead of being
    // refused.
    let (lock, cvar) = &*start_accepting;
    let mut go = lock.lock().unwrap();
    while !*go {
        go = cvar.wait(go).unwrap();
    }
    drop(go);

    for stream in listener.incoming() {
        match stream {
            Ok(stream) => {
                let (event_tx, event_rx) = sync_channel(0);
                event_tx_list.lock().unwrap().push(event_tx);
                thread::spawn({
                    let allowed_urls = allowed_urls.clone();
                    let delivered_tpnote_docs = delivered_tpnote_docs.clone();
                    let session_cookie = session_cookie.clone();
                    let conn_counter = conn_counter.clone();
                    let context = context.clone();
                    move || {
                        let mut st = ServerThread::new(
                            event_rx,
                            stream,
                            allowed_urls,
                            delivered_tpnote_docs,
                            session_cookie,
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
    /// The session token the viewer is bound to, `None` while unbound.
    /// Shared by all server threads (`viewer.session_binding_cookie`).
    pub(crate) session_cookie: Arc<RwLock<Option<String>>>,
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
    /// (`viewer.session_binding_cookie`); cleared by `respond_content_ok()` after
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
        session_cookie: Arc<RwLock<Option<String>>>,
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
            session_cookie,
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

        // Restrict the viewer to the same OS user (`viewer.same_user_policy`),
        // but only while the session is not yet bound: this check guards the
        // bootstrap window (the first-connection race in which a foreign OS
        // user grabs the session during the browser's cold start). Once a
        // session cookie is bound, the cookie gates every request, so skip the
        // O(sockets) peer lookup on later connections; with cookie binding
        // disabled, `session_cookie` stays `None`, so the check runs on every
        // connection, as before (it is then the only OS-user defense).
        #[cfg(feature = "same-user-policy")]
        let session_bound = self.session_cookie.read().is_some();
        #[cfg(feature = "same-user-policy")]
        let local = self.stream.local_addr()?;
        #[cfg(feature = "same-user-policy")]
        let peer = self.stream.peer_addr()?;
        #[cfg(feature = "same-user-policy")]
        match CFG.viewer.same_user_policy {
            SameUserPolicy::Off => {
                // Not checking: say why.
                log::debug!(
                    "TCP port local {} to peer {}: same-user check skipped \
                     (viewer.same_user_policy = Off).",
                    local.port(),
                    peer.port(),
                );
            }
            // Session already bound: the cookie now gates every request, so skip
            // the peer lookup (see the comment above the match).
            _ if session_bound => {}
            policy => {
                // The check just happened: `check` carries the OS user names
                // detected for the local (Tp-Note) process and the peer
                // (viewer) process — the latter is `unknown` when it could not
                // be resolved.
                let check = identify_peer_user(local, peer);
                match check.relation {
                    PeerUser::Same => {
                        // Assertion holds: the peer is our own OS user.
                        log::debug!(
                            "TCP port local {} ({}) to peer {} ({}): Ok, peer is the same OS user.",
                            local.port(),
                            check.local_user,
                            peer.port(),
                            check.peer_user,
                        );
                    }
                    PeerUser::Other => {
                        // Refuse this connection; the viewer keeps running.
                        // The page names the local and viewer users.
                        self.respond_http_error(
                            403,
                            &peer_user_mismatch_page(&check.local_user, &check.peer_user),
                            "peer belongs to a different OS user",
                        )?;
                        return Err(ViewerError::PeerUserMismatch {
                            local_user: check.local_user,
                            peer_user: check.peer_user,
                        });
                    }
                    PeerUser::Unknown => {
                        log::warn!(
                            "TCP port local {} ({}) to peer {} ({}): cannot determine the \
                             connecting client's OS user; same-user check inconclusive.",
                            local.port(),
                            check.local_user,
                            peer.port(),
                            check.peer_user,
                        );
                        // `Warn` serves (fail-open); `Reject` refuses
                        // (fail-closed). Because `Reject` is the default, the
                        // client refused here may well be the legitimate
                        // user's own (sandboxed) browser, so serve the
                        // informative page that explains how to relax to
                        // `Warn` and the risk of doing so.
                        if policy == SameUserPolicy::Reject {
                            self.respond_http_error(
                                403,
                                &peer_user_unknown_page(&check.local_user, &check.peer_user),
                                "peer OS user indeterminate (same_user_policy = Reject)",
                            )?;
                            return Err(ViewerError::PeerUserUnknown {
                                local_user: check.local_user,
                                peer_user: check.peer_user,
                            });
                        }
                    }
                }
            }
        }

        'tcp_connection: loop {
            // This is inspired by the Spook crate.
            // Read the request.
            let mut read_buffer = [0u8; TCP_READ_BUFFER_SIZE];
            let mut buffer = Vec::new();
            let (method, path, cookie, host) = 'assemble_tcp_chunks: loop {
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
                        let cookie: Option<String> = req
                            .headers
                            .iter()
                            .find(|h| h.name.eq_ignore_ascii_case("Cookie"))
                            .and_then(|h| str::from_utf8(h.value).ok())
                            .and_then(parse_tpnote_cookie);
                        let host: Option<String> = req
                            .headers
                            .iter()
                            .find(|h| h.name.eq_ignore_ascii_case("Host"))
                            .and_then(|h| str::from_utf8(h.value).ok())
                            .map(|s| s.trim().to_string());
                        // This is the only regular exit.
                        break 'assemble_tcp_chunks (method, path, cookie, host);
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

            // Session binding (`viewer.session_binding_cookie`), trust-on-first-use:
            // the first navigation (`GET /`) binds the viewer to its client
            // by issuing a random session cookie; once bound, every request
            // must present it. A mismatch refuses that one request and closes
            // its connection — the viewer itself keeps serving the bound
            // client.
            if CFG.viewer.session_binding_cookie {
                let is_navigation = &*path == "/";
                let bound = self.session_cookie.read().clone();
                match bound {
                    // Already bound: every request must present the cookie.
                    Some(tok) => {
                        if cookie.as_deref() != Some(tok.as_str()) {
                            // Refuse the request AND close this connection
                            // (frees its `tcp_connections_max` slot). Never
                            // touch the viewer itself.
                            self.respond_forbidden()?;
                            return Err(ViewerError::SessionCookieMismatch);
                        }
                    }
                    // Unbound.
                    None => {
                        if is_navigation {
                            // This navigation claims the session
                            // (first-writer-wins).
                            let mut w = self.session_cookie.write();
                            if w.is_none() {
                                let tok = new_session_token();
                                *w = Some(tok.clone());
                                // Emit `Set-Cookie` on this response.
                                self.set_cookie = Some(tok);
                            } else if cookie.as_deref() != w.as_deref() {
                                // Lost the race between `read()` and
                                // `write()`: someone bound first and this
                                // navigation lacks the cookie.
                                // Never hold the lock across I/O.
                                drop(w);
                                self.respond_forbidden()?;
                                return Err(ViewerError::SessionCookieMismatch);
                            }
                            // else: raced, but this client already holds the
                            // matching cookie -> fall through.
                        } else if &*path == SSE_EVENT_PATH {
                            // Unbound + `/events`: never legitimate — the
                            // real browser only opens the EventSource AFTER
                            // executing the JS delivered in the bound `/`
                            // page. Serving it would let a foreign client
                            // park on an `event_tx` slot and observe
                            // save-timing metadata. Refuse and close.
                            self.respond_forbidden()?;
                            return Err(ViewerError::SessionCookieMismatch);
                        }
                        // else: unbound + other non-navigation (favicon,
                        // CSS): do NOT bind on it and do NOT 403 it —
                        // otherwise a lone favicon probe pre-binding would
                        // poison the session. Serving it is fail-safe:
                        // `allowed_urls` is empty until the first render and
                        // only `/` (which binds) can trigger a render, so
                        // the most an unbound client gets is the favicon or
                        // CSS.
                    }
                }
            }

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

                // Serve all other documents, including the (binding) `/`
                // navigation.
                _ => {
                    let result = self.respond(&path);
                    // Binding rollback: if this request bound the session
                    // but its response was never delivered (e.g. the note
                    // file was mid-rename during an editor save and the
                    // render failed), the token would stay bound to a client
                    // that never received it — every later request,
                    // including the browser's retry, would be refused
                    // forever. Re-open the binding instead. `set_cookie` is
                    // still `Some` exactly when `respond_content_ok()` never
                    // got to deliver the `Set-Cookie` header.
                    if result.is_err() && self.set_cookie.is_some() {
                        let mut w = self.session_cookie.write();
                        // Compare-before-clear: only if it is still our token.
                        if w.as_deref() == self.set_cookie.as_deref() {
                            *w = None;
                        }
                    }
                    result?
                }
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
    use super::new_session_token;
    use super::parse_tpnote_cookie;

    #[test]
    fn test_new_session_token() {
        let t1 = new_session_token();
        let t2 = new_session_token();
        // `{:02x}` per byte guarantees exactly 32 chars, leading zeros kept.
        assert_eq!(t1.len(), 32);
        assert!(t1.chars().all(|c| c.is_ascii_hexdigit()));
        assert_ne!(t1, t2);
    }

    #[test]
    fn test_parse_tpnote_cookie() {
        assert_eq!(
            parse_tpnote_cookie("a=1; tpnote=deadbeef; b=2").as_deref(),
            Some("deadbeef")
        );
        assert_eq!(parse_tpnote_cookie("tpnote=deadbeef").as_deref(), Some("deadbeef"));
        assert_eq!(parse_tpnote_cookie(" tpnote=x ").as_deref(), Some("x"));
        assert_eq!(parse_tpnote_cookie("nope=1"), None);
        assert_eq!(parse_tpnote_cookie("xtpnote=1"), None);
        assert_eq!(parse_tpnote_cookie(""), None);
    }

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

    /// End-to-end HTTP tests for the viewer's per-request enforcement: the session
    /// cookie binding (`viewer.session_binding_cookie`, TOFU) and the `Host`-header
    /// (DNS-rebinding) check. Each test boots the real [`manage_connections`]
    /// server on an ephemeral loopback port and drives it with a raw TCP client, so
    /// the full `serve_connection2`/`respond` pipeline runs — the part the unit
    /// tests above cannot reach. Because same-process loopback connections resolve
    /// to the same OS user, the default `same_user_policy = Reject` peer check
    /// passes and does not interfere. tpnote is a binary crate (no `lib` target),
    /// so these integration tests live in-module rather than under `tests/`.
    ///
    /// Requires the `renderer` feature (a `GET /` renders the note); it is in the
    /// default feature set.
    #[cfg(feature = "renderer")]
    mod http_integration_tests {
        use super::super::{SseToken, manage_connections};
        use std::fs;
        use std::io::{ErrorKind, Read, Write};
        use std::net::{TcpListener, TcpStream};
        use std::path::PathBuf;
        use std::sync::atomic::{AtomicU32, Ordering};
        use std::sync::mpsc::SyncSender;
        use std::sync::{Arc, Condvar, Mutex};
        use std::thread;
        use std::time::Duration;

        /// A minimal but valid Tp-Note markdown document.
        const NOTE: &str = "---\ntitle: itest\n---\n\nHello integration test.\n";
        /// A 32-hex token that is never the (random) bound token.
        const WRONG_COOKIE: &str = "tpnote=00000000000000000000000000000000";

        static SEQ: AtomicU32 = AtomicU32::new(0);

        /// Boots a viewer server serving a fresh temp note on `127.0.0.1:0` and
        /// returns the bound port. The accept gate is opened immediately. The temp
        /// note and server thread outlive the test (the process reaps them).
        fn boot(note: &str) -> u16 {
            boot_with_doc(note).0
        }

        /// Like [`boot`], but also returns the path of the served note file, so a
        /// test can mutate it (e.g. delete it to force a render failure).
        fn boot_with_doc(note: &str) -> (u16, PathBuf) {
            let listener = TcpListener::bind(("127.0.0.1", 0)).unwrap();
            let port = listener.local_addr().unwrap().port();

            let uniq = SEQ.fetch_add(1, Ordering::Relaxed);
            let dir = std::env::temp_dir().join(format!(
                "tpnote-itest-{}-{}",
                std::process::id(),
                uniq
            ));
            fs::create_dir_all(&dir).unwrap();
            let doc: PathBuf = dir.join("note.md");
            fs::write(&doc, note).unwrap();

            // Accept gate already open (`true`) so the server serves at once.
            let start_accepting = Arc::new((Mutex::new(true), Condvar::new()));
            let event_tx_list: Arc<Mutex<Vec<SyncSender<SseToken>>>> = Arc::new(Mutex::new(Vec::new()));
            let doc_for_server = doc.clone();
            thread::spawn(move || {
                manage_connections(event_tx_list, listener, start_accepting, doc_for_server)
            });
            (port, doc)
        }

        /// Issues one `GET path` request (always with a valid local `Host`) plus any
        /// extra headers, and returns the full raw response. The server keeps the
        /// connection open awaiting a next request, so the reader drains until the
        /// `Content-Length` body is complete or a short timeout elapses.
        fn get(port: u16, path: &str, extra: &[(&str, &str)]) -> String {
            let mut stream = TcpStream::connect(("127.0.0.1", port)).unwrap();
            let mut req = format!("GET {path} HTTP/1.1\r\nHost: localhost:{port}\r\n");
            for (k, v) in extra {
                req.push_str(k);
                req.push_str(": ");
                req.push_str(v);
                req.push_str("\r\n");
            }
            req.push_str("\r\n");
            stream.write_all(req.as_bytes()).unwrap();
            read_response(&stream)
        }

        /// Drains an HTTP response: stops once headers + `Content-Length` bytes are
        /// in, otherwise after the read timeout (covers responses whose connection
        /// the server closes and any without a body).
        fn read_response(stream: &TcpStream) -> String {
            stream
                .set_read_timeout(Some(Duration::from_millis(800)))
                .unwrap();
            let mut stream = stream;
            let mut out: Vec<u8> = Vec::new();
            let mut chunk = [0u8; 4096];
            loop {
                match stream.read(&mut chunk) {
                    Ok(0) => break,
                    Ok(n) => {
                        out.extend_from_slice(&chunk[..n]);
                        if let Some(total) = expected_len(&out)
                            && out.len() >= total
                        {
                            break;
                        }
                    }
                    Err(e)
                        if e.kind() == ErrorKind::WouldBlock || e.kind() == ErrorKind::TimedOut =>
                    {
                        break
                    }
                    Err(_) => break,
                }
            }
            String::from_utf8_lossy(&out).into_owned()
        }

        /// Total expected length (headers + body) once the header block and a
        /// `Content-Length` are present, else `None`.
        fn expected_len(buf: &[u8]) -> Option<usize> {
            let s = std::str::from_utf8(buf).ok()?;
            let head_end = s.find("\r\n\r\n")? + 4;
            let cl = s[..head_end]
                .lines()
                .find(|l| l.to_ascii_lowercase().starts_with("content-length:"))?;
            let n: usize = cl.split(':').nth(1)?.trim().parse().ok()?;
            Some(head_end + n)
        }

        /// Parses the numeric HTTP status code from a raw response.
        fn status(resp: &str) -> u16 {
            resp.split_whitespace()
                .nth(1)
                .and_then(|c| c.parse().ok())
                .unwrap_or(0)
        }

        /// Extracts the `tpnote=` value from a `Set-Cookie` header, if present.
        fn cookie_token(resp: &str) -> Option<String> {
            resp.lines()
                .filter(|l| l.to_ascii_lowercase().starts_with("set-cookie:"))
                .find_map(|l| {
                    let v = l.split_once(':')?.1.trim();
                    v.strip_prefix("tpnote=")
                        .map(|rest| rest.split(';').next().unwrap_or("").to_string())
                })
        }

        #[test]
        fn first_navigation_binds_and_sets_cookie() {
            let port = boot(NOTE);
            let resp = get(port, "/", &[]);
            assert_eq!(status(&resp), 200, "first GET / should render:\n{resp}");
            let tok = cookie_token(&resp).expect("binding response must Set-Cookie tpnote=");
            assert_eq!(tok.len(), 32, "session token is 32 hex chars");
            assert!(resp.contains("HttpOnly"), "cookie must be HttpOnly:\n{resp}");
            assert!(resp.contains("SameSite=Lax"), "cookie must be SameSite=Lax");
        }

        #[test]
        fn bound_request_with_correct_cookie_is_served() {
            let port = boot(NOTE);
            let tok = cookie_token(&get(port, "/", &[])).expect("bind");
            let cookie = format!("tpnote={tok}");
            let resp = get(port, "/", &[("Cookie", cookie.as_str())]);
            assert_eq!(status(&resp), 200, "correct cookie must be served:\n{resp}");
        }

        #[test]
        fn bound_request_without_cookie_is_refused() {
            let port = boot(NOTE);
            let _ = cookie_token(&get(port, "/", &[])).expect("bind");
            // Now bound; a request lacking the cookie is refused.
            let resp = get(port, "/", &[]);
            assert_eq!(status(&resp), 403, "missing cookie once bound => 403:\n{resp}");
        }

        #[test]
        fn bound_request_with_wrong_cookie_is_refused() {
            let port = boot(NOTE);
            let _ = cookie_token(&get(port, "/", &[])).expect("bind");
            let resp = get(port, "/", &[("Cookie", WRONG_COOKIE)]);
            assert_eq!(status(&resp), 403, "wrong cookie => 403:\n{resp}");
        }

        #[test]
        fn foreign_host_header_is_refused() {
            let port = boot(NOTE);
            // A foreign `Host` (DNS-rebinding) is refused before any binding.
            let mut stream = TcpStream::connect(("127.0.0.1", port)).unwrap();
            stream
                .write_all(b"GET / HTTP/1.1\r\nHost: evil.example\r\n\r\n")
                .unwrap();
            let resp = read_response(&stream);
            assert_eq!(status(&resp), 403, "foreign Host => 403:\n{resp}");
        }

        #[test]
        fn missing_host_header_is_allowed() {
            // HTTP/1.0-style client with no `Host` is treated as a local tool.
            let port = boot(NOTE);
            let mut stream = TcpStream::connect(("127.0.0.1", port)).unwrap();
            stream.write_all(b"GET / HTTP/1.1\r\n\r\n").unwrap();
            let resp = read_response(&stream);
            assert_eq!(status(&resp), 200, "missing Host is allowed:\n{resp}");
        }

        #[test]
        fn failed_render_after_binding_rolls_back_binding() {
            // A `GET /` binds the session *before* the note is rendered. If the
            // render then fails (here: the note file vanished mid-request, as an
            // editor save renaming it would cause), the binding must be rolled
            // back — otherwise the viewer would stay bound to a client that never
            // received the cookie and would refuse everyone, including the real
            // browser's retry, forever.
            let (port, doc) = boot_with_doc(NOTE);

            // Delete the note so `ContentString::open` errors after binding.
            std::fs::remove_file(&doc).unwrap();
            // This request binds, then fails to render; the server writes no
            // response and closes the connection once the rollback has run
            // (reaching us as EOF, which synchronises the next step).
            let first = get(port, "/", &[]);
            assert!(
                cookie_token(&first).is_none(),
                "a failed render must not deliver a session cookie:\n{first:?}"
            );

            // Restore the note; the rolled-back binding lets the next client bind.
            std::fs::write(&doc, NOTE).unwrap();
            let second = get(port, "/", &[]);
            assert_eq!(
                status(&second),
                200,
                "after rollback a fresh client must bind and render:\n{second}"
            );
            assert!(
                cookie_token(&second).is_some(),
                "after rollback the next navigation must receive a new cookie:\n{second}"
            );
        }

        #[test]
        fn events_endpoint_refused_before_binding() {
            // An unbound client must not open the SSE stream: doing so would let
            // a foreign client park on an event slot and observe save-timing
            // metadata. Unbound `GET /events` is refused with `403`.
            let port = boot(NOTE);
            let resp = get(port, "/events", &[]);
            assert_eq!(status(&resp), 403, "unbound /events must be refused:\n{resp}");
        }

        #[test]
        fn non_get_method_is_rejected() {
            // Only GET is served; other methods get `405 Method Not Allowed`.
            let port = boot(NOTE);
            let mut stream = TcpStream::connect(("127.0.0.1", port)).unwrap();
            stream
                .write_all(b"POST / HTTP/1.1\r\nHost: localhost\r\n\r\n")
                .unwrap();
            let resp = read_response(&stream);
            assert_eq!(status(&resp), 405, "non-GET must be 405:\n{resp}");
        }

        #[test]
        fn favicon_is_served_without_binding() {
            // The favicon is static and served to an unbound client, but it must
            // NOT bind the session — otherwise a lone favicon probe before the
            // browser's navigation would poison the binding.
            let port = boot(NOTE);
            let fav = get(port, "/favicon.ico", &[]);
            assert_eq!(
                status(&fav),
                200,
                "favicon should be served; status line was {:?}",
                fav.lines().next()
            );
            assert!(cookie_token(&fav).is_none(), "favicon must not bind the session");
            // The session is still unbound, so a fresh navigation still binds.
            let nav = get(port, "/", &[]);
            assert_eq!(status(&nav), 200);
            assert!(
                cookie_token(&nav).is_some(),
                "navigation after a favicon probe must still bind:\n{nav}"
            );
        }

        #[test]
        fn unlisted_path_is_not_served() {
            // Only files referenced by the note (`allowed_urls`) are served; an
            // arbitrary path is refused with `404`, so the viewer cannot be used
            // to read unrelated files.
            let port = boot(NOTE);
            let resp = get(port, "/etc/passwd", &[]);
            assert_eq!(status(&resp), 404, "unlisted path must be 404:\n{resp}");
        }
    }
}
