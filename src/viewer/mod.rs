//! Modules implementing the note content renderer and viewer feature.
pub mod init;
mod sse_server;
mod watcher;
mod web_browser;

use crate::config::LAUNCH_EDITOR;
use crate::viewer::init::Viewer;
use std::path::Path;
use std::thread;

#[inline]
/// Launches a file watcher and Markdown renderer and displays the
/// result in the system's default browser.
pub fn launch_viewer(path: &Path) -> Result<(), anyhow::Error> {
    let p = path.to_path_buf();
    if *LAUNCH_EDITOR {
        thread::spawn(move || Viewer::run(p));
    } else {
        Viewer::run(p);
    }
    Ok(())
}
