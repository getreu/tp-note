//! Server-sent-event server for the note viewer feature.
//! This module contains also the web browser Javascript client code.

use crate::config::CFG;
use crate::config::VIEWER_SERVED_MIME_TYPES_HMAP;
use crate::filter::TERA;
use crate::note::Note;
use crate::viewer::init::LOCALHOST;
use anyhow::anyhow;
use anyhow::Context;
use parse_hyperlinks::iterator_html::{Hyperlink, InlineImage};
use parse_hyperlinks::renderer::text_rawlinks2html;
use percent_encoding::percent_decode_str;
use std::collections::HashSet;
use std::ffi::OsStr;
use std::fs;
use std::io::{ErrorKind, Read, Write};
use std::net::Shutdown;
use std::net::{TcpListener, TcpStream};
use std::path::Path;
use std::path::PathBuf;
use std::str;
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::{Arc, Mutex, RwLock};
use std::thread;
use std::thread::sleep;
use std::time::Duration;
use std::time::SystemTime;
use tera::Tera;
use url::Url;

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
/// Server sent event method name to request a page update.
const SSE_EVENT_NAME: &str = "update";

/// Connection for server sent events.
const EVENT_PATH: &str = "/events";

/// Modern browser request a small icon image.
pub const FAVICON: &[u8] = include_bytes!("favicon.ico");
/// The path where the favicon is requested.
pub const FAVICON_PATH: &str = "/favicon.ico";

/// Chrome and Edge under Windows don't like when the server closes
/// the TCP connection too early and does not wait for ACK.
/// Firefox does not need this.
/// The problem was observed in 4/2020. Maybe some later version
/// does not require this hack.
/// It seems 100ms is enough, we chose a bit more to be sure. This keeps the
/// thread a bit longer alive. The unit is milliseconds.
const SERVER_EXTRA_KEEP_ALIVE: u64 = 900;

pub fn manage_connections(
    event_tx_list: Arc<Mutex<Vec<Sender<()>>>>,
    listener: TcpListener,
    doc_path: PathBuf,
) {
    // `unwarp()` is Ok here here, because we just did it before successfully.
    let sse_port = listener.local_addr().unwrap().port();
    // A list of in the not referenced local links to images or other documents.
    let doc_local_links = Arc::new(RwLock::new(HashSet::new()));
    for stream in listener.incoming() {
        if let Ok(stream) = stream {
            let (event_tx, event_rx) = channel();
            event_tx_list.lock().unwrap().push(event_tx);
            let doc_path = doc_path.clone();
            let doc_local_links = doc_local_links.clone();
            thread::spawn(move || {
                let mut st =
                    ServerThread::new(event_rx, stream, sse_port, doc_path, doc_local_links);
                st.serve_events()
            });
        }
    }
}

/// Server thread state.
struct ServerThread {
    /// Receiver side of the channel where `update` events are sent.
    rx: Receiver<()>,
    /// Byte stream coming from a TCP connection.
    stream: TcpStream,
    /// The TCP port this stream comes from.
    sse_port: u16,
    /// Local file system path of the note document.
    doc_path: PathBuf,
    /// A list of in the not referenced local links to images or other documents.
    doc_local_links: Arc<RwLock<HashSet<PathBuf>>>,
}

impl ServerThread {
    /// Constructor.
    fn new(
        rx: Receiver<()>,
        stream: TcpStream,
        sse_port: u16,
        doc_path: PathBuf,
        doc_local_links: Arc<RwLock<HashSet<PathBuf>>>,
    ) -> Self {
        Self {
            rx,
            stream,
            sse_port,
            doc_path,
            doc_local_links,
        }
    }

    /// Wrapper for `serve_event2()` that prints
    /// errors as log messages on `stderr`.
    fn serve_events(&mut self) {
        match Self::serve_events2(self) {
            Ok(_) => (),
            Err(e) => {
                log::warn!("ServerThread::serve_events(): {:?}", e);
            }
        }
    }

    /// HTTP server: serves events via the specified subscriber stream.
    /// This method also serves the content page and
    /// the content error page.
    #[allow(clippy::needless_return)]
    fn serve_events2(&mut self) -> Result<(), anyhow::Error> {
        // This is inspired by the Spook crate.
        // Read the request.
        let mut read_buffer = [0u8; 512];
        let mut buffer = Vec::new();
        let (method, path) = loop {
            // Read the request, or part thereof.
            match self.stream.read(&mut read_buffer) {
                Ok(0) | Err(_) => {
                    // Connection closed or error.
                    return Ok(());
                }
                Ok(n) => {
                    // Successful read.
                    buffer.extend_from_slice(&read_buffer[..n]);
                }
            }

            // Try to parse the request.
            let mut headers = [httparse::EMPTY_HEADER; 16];
            let mut req = httparse::Request::new(&mut headers);
            match req.parse(&buffer) {
                Ok(_) => {
                    // We are happy even with a partial parse as long as the method
                    // and path are available.
                    if let (Some(method), Some(path)) = (req.method, req.path) {
                        break (method, path);
                    }
                }
                Err(e) => return Err(anyhow!("can not parse request in buffer: {}", e)),
            }
        };
        // End of input junk loop.

        // The only supported request method for SSE is GET.
        if method != "GET" {
            self.stream
                .write_all(b"HTTP/1.1 405 Method Not Allowed\r\n\r\n")?;
            return Ok(());
        }

        // Decode the percent encoding in the URL path.
        let path = percent_decode_str(path)
            .decode_utf8()
            .context(format!("error decoding URL: {}", path))?;

        // Check the path.
        // Serve note rendition.
        if path == "/" {
            let html = Self::render_content_and_error(&self)
                .context("ServerThread::render_content(): ")?;

            let response = format!(
                "HTTP/1.1 200 OK\r\n\
            Cache-Control: no-cache\r\n\
            Date: {}\r\n\
            Content-Type: text/html; charset=utf-8\r\n\
            Content-Length: {}\r\n\r\n",
                httpdate::fmt_http_date(SystemTime::now()),
                html.len()
            );
            self.stream.write_all(response.as_bytes())?;
            self.stream.write_all(html.as_bytes())?;
            // We have been subscribed to events beforehand. As we drop the
            // receiver now, `viewer::update()` will remove us from the list soon.
            log::debug!(
                "ServerThread::serve_events2: 200 OK, served file:\n'{}'",
                self.doc_path.to_str().unwrap_or_default().to_string()
            );
            // Only Chrome and Edge on Windows need this extra time to ACK the TCP
            // connection.
            sleep(Duration::from_millis(SERVER_EXTRA_KEEP_ALIVE));
            self.stream.shutdown(Shutdown::Both)?;
            return Ok(());

        // Serve image.
        } else if path == FAVICON_PATH {
            let response = format!(
                "HTTP/1.1 200 OK\r\n\
            Cache-Control: no-cache\r\n\
            Date: {}\r\n\
            Content-Type: image/x-icon\r\n\
            Content-Length: {}\r\n\r\n",
                httpdate::fmt_http_date(SystemTime::now()),
                FAVICON.len(),
            );
            self.stream.write_all(response.as_bytes())?;
            self.stream.write_all(FAVICON)?;
            log::debug!(
                "ServerThread::serve_events2: 200 OK, served file:\n'{}'",
                FAVICON_PATH
            );
            // Only Chrome and Edge on Windows need this extra time to ACK the TCP
            // connection.
            sleep(Duration::from_millis(SERVER_EXTRA_KEEP_ALIVE));
            self.stream.shutdown(Shutdown::Both)?;
            return Ok(());

        // Serve update events.
        } else if path == EVENT_PATH {
            // This is connection for server sent events.
            // Declare SSE capability and allow cross-origin access.
            let response = format!(
                "\
                HTTP/1.1 200 OK\r\n\
                Access-Control-Allow-Origin: *\r\n\
                Cache-Control: no-cache\r\n\
                Content-Type: text/event-stream\r\n\
                Date: {}\r\n\
                \r\n",
                httpdate::fmt_http_date(SystemTime::now()),
            );
            self.stream.write_all(response.as_bytes())?;

            // Make the stream non-blocking to be able to detect whether the
            // connection was closed by the client.
            self.stream.set_nonblocking(true)?;

            // Serve events until the connection is closed.
            // Keep in mind that the client will often close
            // the request after the first event if the event
            // is used to trigger a page refresh, so try to eagerly
            // detect closed connections.
            loop {
                // Wait for the next update.
                self.rx.recv()?;

                // Detect whether the connection was closed.
                match self.stream.read(&mut read_buffer) {
                    Ok(0) => {
                        // Connection closed.
                        return Ok(());
                    }
                    Ok(_) => {}
                    Err(e) => {
                        if e.kind() != ErrorKind::WouldBlock {
                            // Something bad happened.
                            return Err(anyhow!("error reading stream: {}", e));
                        }
                    }
                }

                // Send event.
                let event = format!("event: {}\r\ndata\r\n\r\n", SSE_EVENT_NAME);
                self.stream.write_all(event.as_bytes())?;
                log::debug!(
                    "ServerThread::serve_events2: 200 OK, served file:\n'{}'",
                    SSE_EVENT_NAME
                );
            }

        // Serve all other documents.
        } else {
            // Strip `/` and convert to `Path`.
            let path = path
                .strip_prefix("/")
                .ok_or_else(|| anyhow!("URL path must start with `/`"))?;
            let reqpath = Path::new(OsStr::new(&path));

            // Condition 1.: Check if we serve this kind of extension
            let extension = &*reqpath
                .extension()
                .unwrap_or_default()
                .to_str()
                .unwrap_or_default();
            // Find the corresponding mime type of this file extension.
            let mime_type = match VIEWER_SERVED_MIME_TYPES_HMAP.get(&*extension) {
                Some(mt) => mt,
                None => {
                    // Reject all files with extensions not listed.
                    log::warn!(
                        "ServerThread::serve_events2: \
                            files with extension '{}' are not served. Rejecting: '{}'",
                        reqpath
                            .extension()
                            .unwrap_or_default()
                            .to_str()
                            .unwrap_or_default(),
                        reqpath.to_str().unwrap_or_default(),
                    );
                    return self.write_not_found(&reqpath);
                }
            };

            // Condition 2.: Only serve files that explicitly appear in `self.doc_local_links`.
            let doc_local_links = self
                .doc_local_links
                .read()
                .map_err(|e| anyhow!("can not obtain RwLock for reading: {}", e))?;
            if !doc_local_links.contains(Path::new(&reqpath)) {
                log::warn!(
                    "ServerThread::serve_events2: target not referenced in note file, rejecting: \
                            '{}'",
                    reqpath.to_str().unwrap_or_default()
                );
                drop(doc_local_links);
                return self.write_not_found(&reqpath);
            }
            // Release the `RwLockReadGuard`.
            drop(doc_local_links);

            // Concatenate document directory and URL path.
            let doc_path = self.doc_path.canonicalize()?;
            let doc_dir = doc_path
                .parent()
                .ok_or_else(|| anyhow!("can not determine document directory"))?;
            // If `path` is absolute, it replaces `doc_dir`.
            let reqpath_abs = doc_dir.join(&reqpath);

            // Condition 3.: Only serve resources in the same or under the document's directory.
            match reqpath_abs.canonicalize() {
                Ok(p) => {
                    if !p.starts_with(doc_dir) {
                        log::warn!(
                            "ServerThread::serve_events2:\
                                file '{}' is not in directory '{}', rejecting.",
                            reqpath.to_str().unwrap_or_default(),
                            doc_dir.to_str().unwrap_or_default()
                        );
                        return self.write_not_found(&reqpath);
                    }
                }
                Err(e) => {
                    log::warn!(
                        "ServerThread::serve_events2: can not access file: \
                            '{}': {}.",
                        reqpath_abs.to_str().unwrap_or_default(),
                        e
                    );
                }
            };

            // Condition 4.: Is the file readable?
            if let Ok(file_content) = fs::read(&reqpath_abs) {
                let response = format!(
                    "HTTP/1.1 200 OK\r\n\
                    Cache-Control: no-cache\r\n\
                    Date: {}\r\n\
                    Content-Type: {}\r\n\
                    Content-Length: {}\r\n\r\n",
                    httpdate::fmt_http_date(SystemTime::now()),
                    mime_type,
                    file_content.len(),
                );
                self.stream.write_all(response.as_bytes())?;
                self.stream.write_all(&file_content)?;
                log::debug!(
                    "ServerThread::serve_events2: 200 OK, served file:\n'{}'",
                    reqpath_abs.to_str().unwrap_or_default()
                );
                // Only Chrome and Edge on Windows need this extra time to ACK the TCP
                // connection.
                sleep(Duration::from_millis(SERVER_EXTRA_KEEP_ALIVE));
                self.stream.shutdown(Shutdown::Both)?;
                return Ok(());
            } else {
                return self.write_not_found(&reqpath);
            }
        };
        // End of serve all other documents.
    }

    /// Write HTTP not found response.
    fn write_not_found(&mut self, file_path: &Path) -> Result<(), anyhow::Error> {
        self.stream.write_all(b"HTTP/1.1 404 Not Found\r\n\r\n")?;
        log::debug!(
            "ServerThread::serve_events2: 404 \"Not found served:\"\n'{}'",
            file_path.to_str().unwrap_or_default()
        );
        Ok(())
    }

    #[inline]
    /// Renders the error page with the `VIEWER_ERROR_TMPL`.
    fn render_content_and_error(&self) -> Result<String, anyhow::Error> {
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
            .context("Error in YAML file header:")
            // Now, try to render to html.
            .and_then(|mut note| {
                note.render_content(file_path_ext, &CFG.viewer_rendition_tmpl, &js)
                    .context("Can not render the note's content:")
            })
            // Now scan the HTML result for links and store them in a HashMap accessible to all threads.
            .and_then(|html| {
                let mut doc_local_links = self
                    .doc_local_links
                    .write()
                    .map_err(|e| anyhow!("Can not obtain RwLock for writing: {}", e))?;

                // Populate the list from scratch.
                doc_local_links.clear();

                // Search for hyperlinks in the HTML rendition of this note.
                for ((_, _, _), (name, link, _)) in Hyperlink::new(&html) {
                    // We skip absolute URLs.
                    if let Ok(url) = Url::parse(&link) {
                        if url.has_host() {
                            continue;
                        };
                    };
                    let path = PathBuf::from(&*percent_decode_str(&link).decode_utf8().context(
                        format!(
                            "Can not decode URL in hyperlink '{}':\n\n{}\n",
                            &name, &link
                        ),
                    )?);
                    // Save the hyperlinks for other threads to check against.
                    doc_local_links.insert(path);
                }
                // Search for image links in the HTML rendition of this note.
                for ((_, _, _), (name, link)) in InlineImage::new(&html) {
                    // We skip absolute URLs.
                    if let Ok(url) = Url::parse(&link) {
                        if url.has_host() {
                            continue;
                        };
                    };
                    let path = PathBuf::from(&*percent_decode_str(&link).decode_utf8().context(
                        format!("Can not decode URL in image '{}':\n\n{}\n", &name, &link),
                    )?);
                    // Save the image links for other threads to check against.
                    doc_local_links.insert(path);
                }

                if doc_local_links.is_empty() {
                    log::debug!(
                        "Viewer: note file has no local hyperlinks. No additional local files are served.",
                    );
                } else {
                    log::info!(
                        "Viewer: referenced and served local files:\n{}",
                        doc_local_links
                        .iter()
                        .map(|p|{
                            let mut s = "*   ".to_string();
                            s.push_str(p.as_path().to_str().unwrap_or_default());
                            s.push_str("\n");
                            s
                        }).collect::<String>()
                    );
                }
                Ok(html)
                // The `RwLockWriteGuard` is released here.
            }) {
            // If the rendition went well, return the HTML.
            Ok(html) => Ok(html),
            // We could not render the note properly. Instead we will render a special error
            // page and return this instead.
            Err(e) => {
                // Render error page providing all information we have.
                let mut context = tera::Context::new();
                let err = format!("{}\n{}", &e, &e.root_cause());
                context.insert("noteError", &err);
                context.insert("file", &self.doc_path.to_str().unwrap_or_default());
                // Java Script
                context.insert("noteJS", &js);

                let note_error_content = fs::read_to_string(&self.doc_path).unwrap_or_default();
                let note_error_content = text_rawlinks2html(&note_error_content);
                context.insert("noteErrorContent", note_error_content.trim());

                let mut tera = Tera::default();
                tera.extend(&TERA)?;
                let html = tera.render_str(&CFG.viewer_error_tmpl, &context)?;
                Ok(html)
            }
        }
    }
}
