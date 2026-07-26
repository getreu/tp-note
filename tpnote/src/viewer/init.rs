//! Main module for the markup renderer and note viewer feature.

use crate::config::CFG;
use crate::settings::ARGS;
use crate::settings::LAUNCH_EDITOR;
use crate::viewer::error::ViewerError;
use crate::viewer::sse_server::SseToken;
use crate::viewer::sse_server::manage_connections;
use crate::viewer::watcher::FileWatcher;
use crate::viewer::web_browser::launch_web_browser;
use std::net::TcpListener;
use std::path::PathBuf;
use std::sync::mpsc::SyncSender;
use std::sync::{Arc, Condvar, Mutex};
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
/// So better just this string. It will be resolved while binding to the TCP
/// port or in the browser when connecting the event source.
pub const LOCALHOST: &str = "localhost";

#[derive(Clone, Default, Debug)]
pub struct Viewer {}

/// Opens the `manage_connections` accept gate when dropped, so that every
/// exit path of `run2()` — including `?` on lines before the explicit
/// `open()` — releases the accept loop. Otherwise an early error would
/// strand a bound listener that never accepts: clients would hang in the
/// TCP backlog forever instead of being served or refused.
struct AcceptGate(Arc<(Mutex<bool>, Condvar)>);

impl AcceptGate {
    fn open(&self) {
        let (lock, cvar) = &*self.0;
        *lock.lock().unwrap() = true;
        cvar.notify_all();
    }
}

impl Drop for AcceptGate {
    fn drop(&mut self) {
        self.open();
    }
}

impl Viewer {
    /// Set up the file watcher, start the `event/html` server and launch web
    /// browser. Returns when the user closes the web browser and/or file
    /// editor. This is a small wrapper printing error messages.
    pub fn run(doc: PathBuf) {
        match Self::run2(doc) {
            Ok(_) => (),
            Err(e) => {
                log::warn!("Viewer::run(): {}", e);
            }
        }
    }

    /// Set up the file watcher, start the `event/html` server and launch
    /// web browser. Returns when the user closes the web browser and/or file
    /// editor.
    #[inline]
    fn run2(doc: PathBuf) -> Result<(), ViewerError> {
        // Check if the master document (note file) has a known file extension.
        match MarkupLanguage::from(&*doc) {
            // A master document with this file extension is exempted from being
            // viewed. We quit here and do not start the viewer.
            MarkupLanguage::RendererDisabled => return Ok(()),
            // This should never happen, since non-Tp-Note files are viewed as
            // text files.
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

        // Gate for the accept loop: `manage_connections` accepts no
        // connection before this gate opens. It is opened immediately
        // before `launch_web_browser` below, which shrinks the
        // accepting-but-unbound window of `viewer.session_binding_cookie` to the
        // browser's cold-start latency — independently of the sign of
        // `viewer.startup_delay`. The port is already bound, so clients
        // connecting early queue in the TCP backlog (no "connection
        // refused").
        let start_accepting = Arc::new((Mutex::new(false), Condvar::new()));
        let accept_gate = AcceptGate(start_accepting.clone());

        // Launch a background HTTP server thread to manage Server-Sent-Event
        // subscribers and to serve the rendered HTML.
        let event_tx_list: Arc<Mutex<Vec<SyncSender<SseToken>>>> = Arc::new(Mutex::new(Vec::new()));
        thread::spawn({
            // Use a separate scope to `clone()`.
            let doc = doc.clone();
            let event_tx_list = event_tx_list.clone();
            let start_accepting = start_accepting.clone();

            move || manage_connections(event_tx_list, listener, start_accepting, doc)
        });

        // Launch the file watcher thread.
        // Send a signal whenever the file is modified. Without error, this thread runs as long as
        // the parent thread (where we are) is running.
        let terminate_on_browser_disconnect = Arc::new(Mutex::new(false));
        let watcher_handle: JoinHandle<_> = thread::spawn({
            let terminate_on_browser_disconnect = terminate_on_browser_disconnect.clone();

            move || match FileWatcher::new(&doc, event_tx_list, terminate_on_browser_disconnect) {
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
        // Open the accept gate BEFORE launching the browser:
        // `launch_web_browser` blocks until the browser process exits — on
        // a cold start that is the whole session — and an `Err` from it
        // must not strand a closed gate on a bound listener.
        accept_gate.open();
        // This may block.
        launch_web_browser(&url)?;
        // Did it?
        if browser_start.elapsed().as_millis() < BROWSER_INSTANCE_MIN_UPTIME {
            // We are here because the browser process did not block.
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
