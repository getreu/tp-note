//! Server-sent-event server for the Markdown note viewer feature.
//! This module contains also the web browser Javascript client code.

use crate::viewer::init::EVENT_PATH;
use std::io::{ErrorKind, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::thread;
const SSE_EVENT_NAME: &str = "update";
use crate::config::ARGS;
use crate::config::CFG;
use crate::filename::MarkupLanguage;
use crate::filter::TERA;
use crate::note::Note;
use crate::viewer::init::LOCALHOST;
use anyhow::anyhow;
use anyhow::Context;
use dissolve::strip_html_tags;
use httpdate;
use pulldown_cmark::{html, Options, Parser};
use rst_parser::parse;
use rst_renderer::render_html;
use std::net::Shutdown;
use std::path::PathBuf;
use std::str;
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

/// Modern browser request a small image.
pub const FAVICON: &[u8] = include_bytes!("favicon.ico");
/// The path where the favicon is requested.
pub const FAVICON_PATH: &str = "/favicon.ico";

pub fn manage_connections(
    event_tx_list: Arc<Mutex<Vec<Sender<()>>>>,
    listener: TcpListener,
    sse_port: u16,
    file_path: PathBuf,
) {
    for stream in listener.incoming() {
        if let Ok(stream) = stream {
            let (event_tx, event_rx) = channel();
            event_tx_list.lock().unwrap().push(event_tx);
            let event_name = SSE_EVENT_NAME.to_string();
            let file_path2 = file_path.clone();
            thread::spawn(move || {
                let mut st = ServerThread::new(event_rx, stream, event_name, sse_port, file_path2);
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
    file_path: PathBuf,
}

impl ServerThread {
    fn new(
        rx: Receiver<()>,
        stream: TcpStream,
        event_name: String,
        sse_port: u16,
        file_path: PathBuf,
    ) -> Self {
        Self {
            rx,
            stream,
            event_name,
            sse_port,
            file_path,
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
                Err(e) => return Err(anyhow!(format!("can not parse request in buffer: {}", e))),
            }
        };

        // The only supported request method for SSE is GET.
        if method != "GET" {
            self.stream
                .write(b"HTTP/1.1 405 Method Not Allowed\r\n\r\n")?;
            return Ok(());
        }

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
                    "*** Debug: ServerThread::serve_events2: file {:?} served.",
                    self.file_path
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
                    "*** Debug: ServerThread::serve_events2: file \"{}\" served.",
                    FAVICON_PATH
                );
            };
            // Only Chrome and Edge on Windows need this extra time to ACK the TCP
            // connection.
            sleep(Duration::from_millis(100));
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
                            return Err(anyhow!(format!("error reading stream: {}", e)));
                        }
                    }
                }

                // Send event.
                let event = format!("event: {}\r\ndata\r\n\r\n", self.event_name);
                self.stream.write(event.as_bytes())?;
                if ARGS.debug {
                    eprintln!(
                        "*** Debug: ServerThread::serve_events2: event: \"{}\" served.",
                        self.event_name
                    );
                };
            }
        } else {
            self.stream.write(b"HTTP/1.1 404 Not Found\r\n\r\n")?;
            if ARGS.debug {
                eprintln!(
                    "*** Debug: ServerThread::serve_events2: Not found: \"{}\" served.",
                    path
                );
            };
            return Ok(());
        }
    }

    #[inline]
    /// Renders the error page with the `VIEWER_ERROR_TMPL`.
    fn render_content_and_error(&self) -> Result<String, anyhow::Error> {
        match Self::render_content(&self) {
            Ok(s) => Ok(s),
            Err(e) => {
                let mut context = tera::Context::new();
                context.insert("noteError", &e.to_string());
                context.insert("file", self.file_path.to_str().unwrap_or_default());
                // Java Script
                let js = format!(
                    "{}{}:{}{}",
                    SSE_CLIENT_CODE1, LOCALHOST, self.sse_port, SSE_CLIENT_CODE2
                );
                context.insert("noteJS", &js);

                let mut tera = Tera::default();
                tera.extend(&TERA)?;
                let html = tera.render_str(&CFG.viewer_error_tmpl, &context)?;
                Ok(html)
            }
        }
    }

    #[inline]
    /// First, determines the markup language from the file extension or
    /// the `fm_file_ext` YAML variable, if present.
    /// Then calls the appropriate markup renderer.
    /// Finally the result is rendered with the `VIEWER_RENDITION_TMPL`
    /// template.
    fn render_content(&self) -> Result<String, anyhow::Error> {
        // Deserialize.
        let mut note = Note::from_existing_note(&self.file_path)?;

        // Register header.
        note.context.insert("fm_all_yaml", note.content.header);

        // Render Body.
        let input = note.content.body;

        // What Markup language is used?

        let ext = match note.context.get("fm_file_ext") {
            Some(tera::Value::String(file_ext)) => Some(file_ext.as_str()),
            _ => None,
        };

        // Render the markup language.
        let html_output = match MarkupLanguage::from(ext, &self.file_path) {
            MarkupLanguage::Markdown => Self::render_md_content(input),
            MarkupLanguage::RestructuredText => Self::render_rst_content(input)?,
            MarkupLanguage::Html => input.to_string(),
            _ => Self::render_txt_content(input),
        };

        // Register rendered body.
        note.context.insert("noteBody", &html_output);

        // Java Script
        let js = format!(
            "{}{}:{}{}",
            SSE_CLIENT_CODE1, LOCALHOST, self.sse_port, SSE_CLIENT_CODE2
        );
        note.context.insert("noteJS", &js);

        let mut tera = Tera::default();
        tera.extend(&TERA)?;
        let html = tera.render_str(&CFG.viewer_rendition_tmpl, &note.context)?;
        Ok(html)
    }

    #[inline]
    /// Markdown renderer.
    fn render_md_content(markdown_input: &str) -> String {
        // Set up options and parser. Besides the CommonMark standard
        // we enable some useful extras.
        let options = Options::all();
        let parser = Parser::new_ext(markdown_input, options);

        // Write to String buffer.
        let mut html_output: String = String::with_capacity(markdown_input.len() * 3 / 2);
        html::push_html(&mut html_output, parser);
        html_output
    }

    #[inline]
    /// RestructuredText renderer.
    fn render_rst_content(rest_input: &str) -> Result<String, anyhow::Error> {
        // To add a newline at the end, we need to copy here. No other choice.
        // This is a work around for:
        // <https://github.com/flying-sheep/rust-rst/issues/30>
        let mut rest_input = rest_input.trim().to_string();
        rest_input.push('\n');
        let document = parse(rest_input.as_str()).map_err(|e| anyhow!(e))?;
        // Write to String buffer.
        let mut html_output: Vec<u8> = Vec::with_capacity(rest_input.len() * 3 / 2);
        //let mut html_output: String = String::with_capacity(rest_input.len() * 3 / 2);
        let _ = render_html(&document, &mut html_output, false);
        Ok(str::from_utf8(&html_output)?.to_string())
    }

    #[inline]
    /// Renderer for markup languages other than the above.
    fn render_txt_content(other_input: &str) -> String {
        let mut html_output = "<pre><code>".to_string();
        html_output.push_str(
            strip_html_tags(other_input)
                .iter()
                .flat_map(|s| s.chars())
                .collect::<String>()
                .as_str(),
        );
        html_output.push_str("</code></pre>");
        html_output
    }
}
