use crate::config::ARGS;
use crate::config::CFG;
use crate::sse_server::manage_connections;
use anyhow::anyhow;
use anyhow::Context;
use notify::{watcher, DebouncedEvent, RecursiveMode, Watcher};
use std::net::SocketAddr;
use std::net::TcpListener;
use std::path::PathBuf;
use std::sync::mpsc::{channel, Sender};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;
use webbrowser::{open_browser, Browser};

pub const EVENT_PATH: &str = "/events";

/// A Server-Sent Event.
#[derive(Clone, Debug)]
pub struct Event {
    pub port: u16,
}

/// Parse result.
#[derive(Clone, Default, Debug)]
pub struct Viewer {
    file: PathBuf,
}

impl Viewer {
    /// Constructor. `file` is the file to watch, render and serve as html.
    pub fn new(file: PathBuf) -> Self {
        Self { file }
    }

    /// Wrapper to start the server. Does not return.
    pub fn run(&self) {
        match Self::run2(self) {
            Ok(_) => (),
            Err(e) => {
                eprintln!("ERROR: Viewer::run(): {:?}", e);
            }
        }
    }

    /// Set up the file watcher and start the event/html server.
    fn run2(&self) -> Result<(), anyhow::Error> {
        // Launch "server sent event" server.
        let event_out = if let Some(p) = ARGS.port {
            Self::get_tcp_listener_at_port(p)?
        } else {
            Self::get_tcp_listener()?
        };

        let notify_period = CFG.viewer_notify_period;

        // Set up the file watcher.
        let (tx, rx) = channel();
        let mut watcher = watcher(tx.clone(), Duration::from_millis(notify_period)).unwrap();
        watcher.watch(&self.file, RecursiveMode::Recursive)?;

        // Launch a background thread to manage server-sent events subscribers.
        let event_tx_list = {
            let (listener, sse_port) = event_out;
            let sse_port = sse_port;
            let file_path = self.file.clone();
            let event_tx_list = Arc::new(Mutex::new(Vec::new()));
            let event_tx_list_clone = event_tx_list.clone();
            thread::spawn(move || {
                manage_connections(event_tx_list_clone, listener, sse_port, file_path)
            });

            event_tx_list
        };

        // Launch webbrowser.
        let url = format!("http://127.0.0.1:{}", event_out.1);
        if ARGS.debug {
            eprintln!(
                "*** Debug: Viewer::run(): launching browser with URL: {}",
                url
            );
        }
        thread::spawn(move || open_browser(Browser::Default, url.as_str()));

        // Send a signal whenever the file is modified.
        loop {
            match rx.recv()? {
                // Ignore rescan and notices.
                DebouncedEvent::NoticeRemove(_)
                | DebouncedEvent::NoticeWrite(_)
                | DebouncedEvent::Rescan => {}

                // Actual modifications.
                DebouncedEvent::Write(_) | DebouncedEvent::Chmod(_) | DebouncedEvent::Create(_) => {
                    // Run the sub-command.
                    Self::update(&event_tx_list)?;
                }

                // Removal or replacement through renaming.
                DebouncedEvent::Remove(path) => {
                    // Instead of modifying the file, some weird editors
                    // (hello Gedit!) remove the current file and recreate it
                    // by renaming the buffer.
                    // To outsmart such editors, the watcher is set up to watch
                    // again a file with the same name. If this succeeds, the
                    // file is deemed changed.
                    watcher
                        .watch(path.clone(), RecursiveMode::NonRecursive)
                        .map_err(|e| anyhow!(e))
                        .and_then(|_| Self::update(&event_tx_list))?
                }

                // Treat renamed files as a fatal error because it may
                // impact the sub-command.
                DebouncedEvent::Rename(_path, _) => {
                    return Err(anyhow!("file was renamed"));
                }

                // Other errors.
                DebouncedEvent::Error(err, _path) => {
                    return Err(err.into());
                }
            }
        }
    }

    /// Run sub-command and notify subscribers.
    fn update(event_tx_list: &Arc<Mutex<Vec<Sender<()>>>>) -> Result<(), anyhow::Error> {
        // Notify subscribers and forget disconnected subscribers.
        let tx_list = &mut *event_tx_list.lock().unwrap();
        *tx_list = tx_list.drain(..).filter(|tx| tx.send(()).is_ok()).collect();
        if ARGS.debug {
            println!(
                "*** Debug: Viewer::update(): {} subscribers updated.",
                tx_list.len()
            );
        };
        Ok(())
    }
    /// Get TCP port and bind.
    fn get_tcp_listener_at_port(port: u16) -> Result<(TcpListener, u16), anyhow::Error> {
        TcpListener::bind(SocketAddr::from(([127, 0, 0, 1], port)))
            .map(|l| (l, port))
            .with_context(|| format!("can not bind to port: {}", port))
    }

    /// Get TCP port and bind.
    fn get_tcp_listener() -> Result<(TcpListener, u16), anyhow::Error> {
        // Some randomness to better hide this port.
        let mut start = rand::random::<u16>() & 0x8fff;
        if start <= 1024 {
            start += 1024
        };
        for port in start..0xffff {
            match TcpListener::bind(SocketAddr::from(([127, 0, 0, 1], port))) {
                Ok(l) => return Ok((l, port)),
                _ => {}
            }
        }

        Err(anyhow!("can not find free port to bind to"))
    }
}
