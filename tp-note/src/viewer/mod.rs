//! Modules implementing the note content renderer and viewer feature.
mod error;
mod http_response;
pub mod init;
mod sse_server;
mod watcher;
mod web_browser;

use crate::viewer::init::Viewer;
use std::path::Path;
use std::thread;
use std::thread::JoinHandle;

#[inline]
/// Launches a file watcher and Markdown renderer and displays the
/// result in the system's default browser.
pub fn launch_viewer_thread(path: &Path) -> JoinHandle<()> {
    let p = path.to_path_buf();
    thread::spawn(move || Viewer::run(p))
}
