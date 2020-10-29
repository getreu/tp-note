use crate::viewer::EVENT_PATH;
use std::io::{ErrorKind, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::thread;
const SSE_EVENT_NAME: &str = "update";
use crate::config::CFG;
use crate::filter::TERA;
use crate::note::Note;
use anyhow::anyhow;
use anyhow::Context;
use pulldown_cmark::{html, Options, Parser};
use std::path::PathBuf;
use tera::Tera;

/// Javascript client code, part 1
/// Refresh on WTFiles events.
pub const SSE_CLIENT_CODE1: &str = r#"
var evtSource = new EventSource("http://127.0.0.1:"#;
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

// Listen for SSE requests.
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
    /// errors as log messages.
    fn serve_events(&mut self) {
        match Self::serve_events2(self) {
            Ok(_) => (),
            Err(e) => {
                eprintln!("ERROR: ServerThread::serve_events(): {:?}", e);
            }
        }
    }

    /// Serve events via the specified subscriber stream.
    /// This method also servers the content page and
    /// the content error page.
    fn serve_events2(&mut self) -> Result<(), anyhow::Error> {
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
            let html = Self::render_content(&self).context("ServerThread::render_content(): ")?;

            let response = format!(
                "HTTP/1.1 200 OK\r\n\
            Connection: Keep-Alive\r\n\
            Content-Type: text/html; charset=utf-8\r\n\
            Content-Length: {}\r\n\r\n{}",
                html.len(),
                html
            );
            self.stream.write(response.as_bytes())?;
            self.stream.flush()?;
            // We have been subscribed to events beforehand. As we drop the
            // receiver now, `viewer::update()` will remove us from the list soon.
            return Ok(());
        } else if path != EVENT_PATH {
            self.stream.write(b"HTTP/1.1 404 Not Found\r\n\r\n")?;
            return Ok(());
        }

        // This is connection for server sent events.
        // Declare SSE capability and allow cross-origin access.
        let response = b"\
        HTTP/1.1 200 OK\r\n\
        Access-Control-Allow-Origin: *\r\n\
        Cache-Control: no-cache\r\n\
        Connection: keep-alive\r\n\
        Content-Type: text/event-stream\r\n\
        \r\n";
        self.stream.write(response)?;

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
        }
    }

    fn render_content(&self) -> Result<String, anyhow::Error> {
        // Deserialize.
        let mut note = match Note::from_existing_note(&self.file_path) {
            Ok(n) => n,
            Err(e) => {
                let mut context = tera::Context::new();
                context.insert("noteError", &e.to_string());
                context.insert("file", self.file_path.to_str().unwrap_or_default());
                // Java Script
                let js = format!("{}{}{}", SSE_CLIENT_CODE1, self.sse_port, SSE_CLIENT_CODE2);
                context.insert("noteJS", &js);

                let mut tera = Tera::default();
                tera.extend(&TERA)?;
                let html = tera.render_str(&CFG.viewer_error_tmpl, &context)?;
                return Ok(html);
            }
        };

        // Register header.
        note.context.insert("fm_all_yaml", note.content.header);

        // Render Markdown Body
        let markdown_input = note.content.body;
        // Set up options and parser. Besides the CommonMark standard
        // we enable some useful extras.
        let options = Options::all();
        let parser = Parser::new_ext(markdown_input, options);

        // Write to String buffer.
        let mut html_output: String = String::with_capacity(markdown_input.len() * 3 / 2);
        html::push_html(&mut html_output, parser);
        // Register rendered body.
        note.context.insert("noteBody", &html_output);

        // Java Script
        let js = format!("{}{}{}", SSE_CLIENT_CODE1, self.sse_port, SSE_CLIENT_CODE2);
        note.context.insert("noteJS", &js);

        let mut tera = Tera::default();
        tera.extend(&TERA)?;
        let html = tera.render_str(&CFG.viewer_rendition_tmpl, &note.context)?;
        Ok(html)
    }
}
