//! Server-sent-event server for the note viewer feature.
//! This module contains also the web browser Javascript client code.

use crate::config::ARGS;
use crate::config::CFG;
use crate::config::VIEWER_SERVED_MIME_TYPES_HMAP;
use crate::filter::TERA;
use crate::note::Note;
use crate::viewer::init::LOCALHOST;
use anyhow::anyhow;
use anyhow::Context;
use httpdate;
use parse_hyperlinks::renderer::text_rawlinks2html;
use percent_encoding::percent_decode_str;
use std::ffi::OsStr;
use std::fs;
use std::io::{ErrorKind, Read, Write};
use std::net::Shutdown;
use std::net::{TcpListener, TcpStream};
use std::path::Path;
use std::path::PathBuf;
use std::str;
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::thread;
use std::thread::sleep;
use std::time::Duration;
use std::time::SystemTime;
use tera::Tera;

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

pub fn manage_connections(
    event_tx_list: Arc<Mutex<Vec<Sender<()>>>>,
    listener: TcpListener,
    doc_path: PathBuf,
) {
    // Unwarp is Ok here here, because we just did it before successfully.
    let sse_port = listener.local_addr().unwrap().port();
    for stream in listener.incoming() {
        if let Ok(stream) = stream {
            let (event_tx, event_rx) = channel();
            event_tx_list.lock().unwrap().push(event_tx);
            let event_name = SSE_EVENT_NAME.to_string();
            let doc_path2 = doc_path.clone();
            thread::spawn(move || {
                let mut st = ServerThread::new(event_rx, stream, event_name, sse_port, doc_path2);
                st.serve_events()
            });
        }
    }
}

struct ServerThread {
    rx: Receiver<()>,
    stream: TcpStream,
    event_name: String,
    sse_port: u16,
    doc_path: PathBuf,
}

impl ServerThread {
    fn new(
        rx: Receiver<()>,
        stream: TcpStream,
        event_name: String,
        sse_port: u16,
        doc_path: PathBuf,
    ) -> Self {
        Self {
            rx,
            stream,
            event_name,
            sse_port,
            doc_path,
        }
    }

    /// Wrapper for `serve_event2()` that prints
    /// errors as log messages on `stderr`.
    fn serve_events(&mut self) {
        match Self::serve_events2(self) {
            Ok(_) => (),
            Err(e) => {
                eprintln!("ERROR: ServerThread::serve_events(): {:?}", e);
            }
        }
    }

    /// HTTP server: serves events via the specified subscriber stream.
    /// This method also serves the content page and
    /// the content error page.
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

        // The only supported request method for SSE is GET.
        if method != "GET" {
            self.stream
                .write(b"HTTP/1.1 405 Method Not Allowed\r\n\r\n")?;
            return Ok(());
        }

        // Decode the percent encoding in the URL path.
        let path = percent_decode_str(path).decode_utf8()?;

        // Check the path.
        // The browser requests the content.
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
            self.stream.write(response.as_bytes())?;
            self.stream.write(html.as_bytes())?;
            // We have been subscribed to events beforehand. As we drop the
            // receiver now, `viewer::update()` will remove us from the list soon.
            if ARGS.debug {
                eprintln!(
                    "*** Debug: ServerThread::serve_events2: 200 OK, file {:?} served.",
                    self.doc_path
                );
            }
            // Only Chrome and Edge on Windows need this extra time to ACK the TCP
            // connection.
            sleep(Duration::from_millis(100));
            self.stream.shutdown(Shutdown::Both)?;
            return Ok(());
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
            self.stream.write(response.as_bytes())?;
            self.stream.write(FAVICON)?;
            if ARGS.debug {
                eprintln!(
                    "*** Debug: ServerThread::serve_events2: 200 OK, file \"{}\" served.",
                    FAVICON_PATH
                );
            };
            // Only Chrome and Edge on Windows need this extra time to ACK the TCP
            // connection.
            sleep(Duration::from_millis(900));
            self.stream.shutdown(Shutdown::Both)?;
            return Ok(());
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
            self.stream.write(response.as_bytes())?;

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
                let event = format!("event: {}\r\ndata\r\n\r\n", self.event_name);
                self.stream.write(event.as_bytes())?;
                if ARGS.debug {
                    eprintln!(
                        "*** Debug: ServerThread::serve_events2: 200 OK, event \"{}\" served.",
                        self.event_name
                    );
                };
            }
        } else {
            // Strip `/` and convert to `Path`.
            let path = path
                .strip_prefix("/")
                .ok_or_else(|| anyhow!("URL path must start with `/`"))?;
            let path = Path::new(OsStr::new(&path));
            // Concatenate document directory and URL path.
            let doc_path = self.doc_path.canonicalize()?;
            let doc_dir = doc_path
                .parent()
                .ok_or_else(|| anyhow!("can not determine document directory"))?;
            let file_path = doc_dir.join(path);

            let extension = file_path
                .extension()
                .unwrap_or_default()
                .to_str()
                .unwrap_or_default();

            // Find the corresponding mime type of this file extension.
            let mime_type = match VIEWER_SERVED_MIME_TYPES_HMAP.get(&*extension) {
                Some(mt) => mt,
                None => {
                    // Reject all files with extensions not listed.
                    if ARGS.debug {
                        eprintln!(
                            "*** Debug: ServerThread::serve_events2: \
                            file type of \"{}\" is not served, rejecting.",
                            path.to_str().unwrap_or_default(),
                        );
                    };
                    return self.write_not_found(&path);
                }
            };

            // Only serve resources in the same or under the document's directory.
            match file_path.canonicalize() {
                Ok(p) => {
                    if !p.starts_with(doc_dir) {
                        if ARGS.debug {
                            eprintln!(
                                "*** Debug: ServerThread::serve_events2:\
                                file \"{}\" is not in directory \"{}\", rejecting.",
                                path.to_str().unwrap_or_default(),
                                doc_dir.to_str().unwrap_or_default()
                            );
                            return self.write_not_found(&path);
                        };
                    }
                }
                Err(e) => {
                    if ARGS.debug {
                        eprintln!(
                            "*** Debug: ServerThread::serve_events2: can not access file: \
                            \"{}\": {}.",
                            file_path.to_str().unwrap_or_default(),
                            e
                        );
                    };
                }
            };

            if let Ok(file_content) = fs::read(&file_path) {
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
                self.stream.write(response.as_bytes())?;
                self.stream.write(&file_content)?;
                if ARGS.debug {
                    eprintln!(
                        "*** Debug: ServerThread::serve_events2: 200 OK, file \"{}\" served.",
                        file_path.to_str().unwrap_or_default()
                    );
                };
                // Only Chrome and Edge on Windows need this extra time to ACK the TCP
                // connection.
                sleep(Duration::from_millis(900));
                self.stream.shutdown(Shutdown::Both)?;
                return Ok(());
            } else {
                return self.write_not_found(&path);
            }
        }
    }

    /// Write HTTP not found response.
    fn write_not_found(&mut self, file_path: &Path) -> Result<(), anyhow::Error> {
        self.stream.write(b"HTTP/1.1 404 Not Found\r\n\r\n")?;
        if ARGS.debug {
            eprintln!(
                "*** Debug: ServerThread::serve_events2: 404 Not found, \"{}\" served.",
                file_path.to_str().unwrap_or_default()
            );
        };
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
        match Note::from_existing_note(&self.doc_path).and_then(|mut note| {
            note.render_content(file_path_ext, &CFG.viewer_rendition_tmpl, &js)
        }) {
            Ok(s) => Ok(s),
            Err(e) => {
                // Render error page providing all information we have.
                let mut context = tera::Context::new();
                context.insert("noteError", &e.to_string());
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
