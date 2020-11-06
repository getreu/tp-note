//! Main module for the Markdown note viewer feature.

use crate::config::ARGS;
use crate::config::LAUNCH_EDITOR;
use crate::filename::MarkupLanguage;
use crate::sse_server::manage_connections;
use crate::watcher::FileWatcher;
use anyhow::anyhow;
use anyhow::Context;
use std::net::TcpListener;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::thread;
use std::thread::sleep;
use std::thread::JoinHandle;
use std::time::{Duration, Instant};
use webbrowser::{open_browser, Browser};
use std::net::IpAddr;
use std::net::Ipv4Addr;


pub const EVENT_PATH: &str = "/events";
pub const LOCALHOST: IpAddr = IpAddr::V4(Ipv4Addr::LOCALHOST);

/// Parse result.
#[derive(Clone, Default, Debug)]
pub struct Viewer {}

impl Viewer {
    /// Set up the file watcher, start the event/html server and lauch web browser.
    /// Returns when the use closes the webbrowswer.
    /// This is a small wrapper, that prints error messages.
    pub fn run(file: PathBuf) {
        match Self::run2(file) {
            Ok(_) => (),
            Err(e) => {
                eprintln!("ERROR: Viewer::run(): {:?}", e);
            }
        }
    }

    /// Set up the file watcher, start the event/html server and lauch web browser.
    /// Returns when the use closes the webbrowswer.
    fn run2(file: PathBuf) -> Result<(), anyhow::Error> {
        match (ARGS.view, MarkupLanguage::from(None, &file)) {
            // The file with this file extension is exempted from being viewed.
            // We quit here and do not start the viewer.
            (false, MarkupLanguage::Unknown) => return Ok(()),
            // This should never happen, since non-Tp-Note files are never
            // edited or viewed.
            (_, MarkupLanguage::None) => return Err(anyhow!("can not view non Tp-Note files")),
            // All other cases: start viewer.
            (_, _) => (),
        };

        // Launch "server sent event" server.
        let event_out = if let Some(p) = ARGS.port {
            Self::get_tcp_listener_at_port(p)?
        } else {
            Self::get_tcp_listener()?
        };

        // Launch a background thread to manage server-sent events subscribers.
        let event_tx_list = {
            let (listener, sse_port) = event_out;
            let sse_port = sse_port;
            let file_path = file.clone();
            let event_tx_list = Arc::new(Mutex::new(Vec::new()));
            let event_tx_list_clone = event_tx_list.clone();
            thread::spawn(move || {
                manage_connections(event_tx_list_clone, listener, sse_port, file_path)
            });

            event_tx_list
        };

        // Send a signal whenever the file is modified. This thread runs as
        // long as the parent thread is running.
        let handle: JoinHandle<Result<(), anyhow::Error>> = thread::spawn(move || loop {
            let mut w = FileWatcher::new(file.clone(), event_tx_list.clone());
            w.run()
        });

        // Launch webbrowser.
        let url = format!("http://{}:{}", LOCALHOST, event_out.1);
        if ARGS.debug {
            eprintln!(
                "*** Debug: Viewer::run(): launching browser with URL: {}",
                url
            );
        }
        // This blocks when a new instance of the browser is opened.
        let now = Instant::now();
        open_browser(Browser::Default, url.as_str())?;
        // Some browsers do not block, then we wait a little
        // to give him time read the page.
        if now.elapsed().as_secs() <= 4 {
            sleep(Duration::new(4, 0));
        };

        if *LAUNCH_EDITOR {
            // We keep this thread alive as long as the watcher thread
            // is running. As the watcher never ends, the `join()`
            // will block forever unless the parent thread terminates.
            // The parent thread and this tread will finally end, when
            // the user closes the external file editor programm.
            handle.join().unwrap()
        } else {
            // In "view-only" mode, there is no external text editor to
            // wait for. We are here, because the user just closed the
            // browswer. So it is Ok to exit now. This also terminates
            // the Sse-server.
            Ok(())
        }
    }

    /// Get TCP port and bind.
    fn get_tcp_listener_at_port(port: u16) -> Result<(TcpListener, u16), anyhow::Error> {
        TcpListener::bind((LOCALHOST, port))
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
            match TcpListener::bind((LOCALHOST, port)) {
                Ok(l) => return Ok((l, port)),
                _ => {}
            }
        }

        Err(anyhow!("can not find free port to bind to"))
    }
}
