//! Main module for the markup renderer and note viewer feature.

use crate::config::ARGS;
use crate::config::LAUNCH_EDITOR;
use crate::filename::MarkupLanguage;
use crate::viewer::sse_server::manage_connections;
use crate::viewer::watcher::FileWatcher;
use anyhow::anyhow;
use std::net::TcpListener;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::thread;
use std::thread::sleep;
use std::thread::JoinHandle;
use std::time::{Duration, Instant};
use webbrowser::{open_browser, Browser};

pub const EVENT_PATH: &str = "/events";

/// This is where our loop back device is.
/// The following is also possible, but binds us to IPv4:
/// `pub const LOCALHOST: IpAddr = IpAddr::V4(Ipv4Addr::LOCALHOST);`
/// So better just this string. It will be resolved while binding
/// to the TCP port and also in the browser when connecting the
/// event source.
pub const LOCALHOST: &str = "localhost";
/// Parse result.
#[derive(Clone, Default, Debug)]
pub struct Viewer {}

impl Viewer {
    /// Set up the file watcher, start the event/html server and launch web browser.
    /// Returns when the user closes the web browser.
    /// This is a small wrapper, that prints error messages.
    pub fn run(file: PathBuf) {
        match Self::run2(file) {
            Ok(_) => (),
            Err(e) => {
                eprintln!("ERROR: Viewer::run(): {:?}", e);
            }
        }
    }

    /// Set up the file watcher, start the event/html server and launch web browser.
    /// Returns when the use closes the web browser.
    fn run2(file: PathBuf) -> Result<(), anyhow::Error> {
        match (
            ARGS.view,
            MarkupLanguage::from(
                None,
                file.extension()
                    .unwrap_or_default()
                    .to_str()
                    .unwrap_or_default(),
            ),
        ) {
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
        let listener = if let Some(p) = ARGS.port {
            TcpListener::bind((LOCALHOST, p))?
        } else {
            // Use random port.
            TcpListener::bind((LOCALHOST, 0))?
        };
        let localport = listener.local_addr()?.port();

        // Launch a background thread to manage server-sent events subscribers.
        let event_tx_list = {
            let file_path = file.clone();
            let event_tx_list = Arc::new(Mutex::new(Vec::new()));
            let event_tx_list_clone = event_tx_list.clone();
            thread::spawn(move || manage_connections(event_tx_list_clone, listener, file_path));

            event_tx_list
        };

        // Send a signal whenever the file is modified. This thread runs as
        // long as the parent thread is running.
        let event_tx_list_clone = event_tx_list.clone();
        let handle: JoinHandle<Result<(), anyhow::Error>> = thread::spawn(move || loop {
            let mut w = FileWatcher::new(file.clone(), event_tx_list_clone.clone());
            w.run()
        });

        // Launch web browser.
        let url = format!("http://{}:{}", LOCALHOST, localport);
        if ARGS.debug {
            eprintln!(
                "*** Debug: Viewer::run(): launching browser with URL: {}",
                url
            );
        }
        // This blocks when a new instance of the browser is opened.
        let now = Instant::now();
        open_browser(Browser::Default, url.as_str())?;
        FileWatcher::update(&event_tx_list);
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
            // the user closes the external file editor program.
            handle.join().unwrap()
        } else {
            // In "view-only" mode, there is no external text editor to
            // wait for. We are here, because the user just closed the
            // browser. So it is Ok to exit now. This also terminates
            // the Sse-server.
            Ok(())
        }
    }
}