//! Main module for the markup renderer and note viewer feature.

use crate::config::ARGS;
use crate::config::CFG;
use crate::config::VIEWER_SERVED_MIME_TYPES_HMAP;
use crate::filename::MarkupLanguage;
use crate::viewer::sse_server::manage_connections;
use crate::viewer::watcher::FileWatcher;
use crate::viewer::web_browser;
use anyhow::anyhow;
use std::net::TcpListener;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::thread;
use std::thread::JoinHandle;
use web_browser::launch_web_browser;

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
                log::warn!("Viewer::run(): {:?}", e);
            }
        }
    }

    /// Set up the file watcher, start the event/html server and launch web browser.
    /// Returns when the user closes the web browser and/or file editor.
    #[inline]
    fn run2(doc: PathBuf) -> Result<(), anyhow::Error> {
        // Check if the master document (note file) has a known file extension.
        match (
            ARGS.view,
            MarkupLanguage::from(
                None,
                doc.extension()
                    .unwrap_or_default()
                    .to_str()
                    .unwrap_or_default(),
            ),
        ) {
            // A master document with this file extension is exempted from being viewed.
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

        // Concerning non-master-documents, we only serve these file extensions.
        lazy_static::initialize(&VIEWER_SERVED_MIME_TYPES_HMAP);
        log::debug!(
                "Viewer::run(): \
                 Besides the note's HTML rendition, we only serve files with the following listed extensions:\n{:?}", CFG.viewer_served_mime_types
            );

        // Launch a background HTTP server thread to manage server-sent events subscribers
        // and to serve the rendered html.
        let event_tx_list = {
            let doc_path = doc.clone();
            let event_tx_list = Arc::new(Mutex::new(Vec::new()));
            let event_tx_list_clone = event_tx_list.clone();
            thread::spawn(move || manage_connections(event_tx_list_clone, listener, doc_path));

            event_tx_list
        };

        // Launch the file watcher thread.
        // Send a signal whenever the file is modified. Without error, this thread runs as long as
        // the parent thread (where we are) is running.
        let event_tx_list_clone = event_tx_list.clone();
        let _handle: JoinHandle<_> = thread::spawn(move || loop {
            match FileWatcher::new(doc.clone(), event_tx_list_clone.clone()) {
                Ok(mut w) => w.run(),
                Err(e) => {
                    log::warn!("Can not (re-)start file watcher, giving up: {}", e);
                    break;
                }
            }
        });

        // Launch web browser.
        let url = format!("http://{}:{}", LOCALHOST, localport);
        log::info!("Viewer::run(): launching browser with URL: {}", url);

        // This blocks until the browser is closed.
        launch_web_browser(&url)?;

        // This will also terminate the `FileWatcher` thread and web server thread.
        Ok(())
    }
}
