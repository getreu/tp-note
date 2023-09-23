//! Main module for the markup renderer and note viewer feature.

use crate::config::CFG;
use crate::config::VIEWER_SERVED_MIME_TYPES_MAP;
use crate::settings::ARGS;
use crate::settings::LAUNCH_EDITOR;
use crate::viewer::error::ViewerError;
use crate::viewer::sse_server::manage_connections;
use crate::viewer::sse_server::SseToken;
use crate::viewer::watcher::FileWatcher;
use crate::viewer::web_browser::launch_web_browser;
use std::net::TcpListener;
use std::path::PathBuf;
use std::sync::mpsc::SyncSender;
use std::sync::{Arc, Mutex};
use std::thread;
use std::thread::JoinHandle;
use std::time::Duration;
use std::time::Instant;
use tpnote_lib::markup_language::MarkupLanguage;

/// Minimum uptime in milliseconds we expect a real browser instance to run.
/// When starting a second browser instance, only a signal is sent to the
/// first instance and the process returns immediately. We detect this
/// case if it runs less milliseconds than this constant.
const BROWSER_INSTANCE_MIN_UPTIME: u128 = 3000;

/// This is where our loop back device is.
/// The following is also possible, but binds us to IPv4:
/// `pub const LOCALHOST: IpAddr = IpAddr::V4(Ipv4Addr::LOCALHOST);`
/// So better just this string. It will be resolved while binding
/// to the TCP port and also in the browser when connecting the
/// event source.
pub const LOCALHOST: &str = "localhost";

#[derive(Clone, Default, Debug)]
pub struct Viewer {}

impl Viewer {
    /// Set up the file watcher, start the event/html server and launch web browser.
    /// Returns when the user closes the web browser and/or file editor.
    /// This is a small wrapper printing error messages.
    pub fn run(doc: PathBuf) {
        match Self::run2(doc) {
            Ok(_) => (),
            Err(e) => {
                log::warn!("Viewer::run(): {}", e);
            }
        }
    }

    /// Set up the file watcher, start the event/html server and launch web browser.
    /// Returns when the user closes the web browser and/or file editor.
    #[inline]
    fn run2(doc: PathBuf) -> Result<(), ViewerError> {
        // Check if the master document (note file) has a known file extension.
        match MarkupLanguage::from(
            doc.extension()
                .unwrap_or_default()
                .to_str()
                .unwrap_or_default(),
        ) {
            // A master document with this file extension is exempted from being viewed.
            // We quit here and do not start the viewer.
            MarkupLanguage::Unknown => return Ok(()),
            // This should never happen, since non-Tp-Note files are viewed as text files.
            MarkupLanguage::None => return Err(ViewerError::MarkupLanguageNone),
            // All other cases: start viewer.
            _ => (),
        };

        // Launch "server sent event" server.
        let listener = if let Some(p) = ARGS.port {
            TcpListener::bind((LOCALHOST, p))?
        } else {
            // Use random port.
            TcpListener::bind((LOCALHOST, 0))?
        };
        let localport = listener.local_addr()?.port();

        // We only serve files with `VIEWER_SERVED_MIME_TYPES_HMAP` file extensions.
        lazy_static::initialize(&VIEWER_SERVED_MIME_TYPES_MAP);

        // Launch a background HTTP server thread to manage Server-Sent-Event subscribers
        // and to serve the rendered html.
        let event_tx_list: Arc<Mutex<Vec<SyncSender<SseToken>>>> = Arc::new(Mutex::new(Vec::new()));
        thread::spawn({
            // Use a separate scope to `clone()`.
            let doc = doc.clone();
            let event_tx_list = event_tx_list.clone();

            move || manage_connections(event_tx_list, listener, doc)
        });

        // Launch the file watcher thread.
        // Send a signal whenever the file is modified. Without error, this thread runs as long as
        // the parent thread (where we are) is running.
        let terminate_on_browser_disconnect = Arc::new(Mutex::new(false));
        let watcher_handle: JoinHandle<_> = thread::spawn({
            let terminate_on_browser_disconnect = terminate_on_browser_disconnect.clone();

            move || match FileWatcher::new(doc, event_tx_list, terminate_on_browser_disconnect) {
                Ok(mut w) => w.run(),
                Err(e) => {
                    log::warn!("Can not start file watcher, giving up: {}", e);
                }
            }
        });

        // Launch web browser.
        let url = format!("http://{}:{}", LOCALHOST, localport);

        // Shall the browser be started a little later?
        if CFG.viewer.startup_delay > 0 {
            thread::sleep(Duration::from_millis(CFG.viewer.startup_delay as u64));
        };
        // Start timer.
        let browser_start = Instant::now();
        // This may block.
        launch_web_browser(&url)?;
        // Did it?
        if browser_start.elapsed().as_millis() < BROWSER_INSTANCE_MIN_UPTIME {
            // We are there because the browser process did not block.
            // We instruct the watcher to terminate when it detects browser disconnection.
            if !*LAUNCH_EDITOR {
                // Release lock immediately.
                *terminate_on_browser_disconnect.lock().unwrap() = true;
            };
            watcher_handle.join().unwrap();
        }

        Ok(())
    }
}
