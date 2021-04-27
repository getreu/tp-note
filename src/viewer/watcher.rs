//! Implements the file watcher for the note viewer feature.

extern crate notify;

use crate::config::CFG;
use crate::viewer::error::ViewerError;
use notify::{watcher, DebouncedEvent, RecommendedWatcher, RecursiveMode, Watcher};
use std::path::PathBuf;
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::thread::sleep;
use std::time::Duration;

/// Some file editors do not modify the file on disk, they move the old version
/// away and write a new file with the same name. As this takes some time,
/// the watcher waits a bit before trying to access the new file.
/// The delay is in milliseconds.
const WAIT_EDITOR_WRITING_NEW_FILE: u64 = 200;

/// The `watcher` notifies about changes through `rx`.
pub struct FileWatcher {
    // Receiver for file changed messages.
    rx: Receiver<DebouncedEvent>,
    // File watcher.
    watcher: RecommendedWatcher,
    // List of subscribers to inform when the file is changed.
    event_tx_list: Arc<Mutex<Vec<Sender<()>>>>,
}

/// Watch file changes and notify subscribers.
impl FileWatcher {
    /// Constructor. `file` is the file to watch.
    pub fn new(
        file: PathBuf,
        event_tx_list: Arc<Mutex<Vec<Sender<()>>>>,
    ) -> Result<Self, ViewerError> {
        let notify_period = CFG.viewer_notify_period;
        let (tx, rx) = channel();
        let mut watcher = watcher(tx, Duration::from_millis(notify_period))?;
        watcher.watch(&file, RecursiveMode::Recursive)?;
        log::debug!("File watcher started");

        Ok(Self {
            rx,
            watcher,
            event_tx_list,
        })
    }

    /// Wrapper to start the server. Does not return.
    pub fn run(&mut self) {
        match Self::run2(self) {
            Ok(_) => (),
            Err(e) => {
                log::debug!("File watcher terminated: {}", e);
            }
        }
    }

    /// Set up the file watcher and start the event/html server.
    fn run2(&mut self) -> Result<(), ViewerError> {
        loop {
            let evnt = self.rx.recv().unwrap();
            log::trace!("File watcher event: {:?}", evnt);

            match evnt {
                DebouncedEvent::NoticeRemove(path) | DebouncedEvent::Remove(path) => {
                    // Some text editors e.g. Neovim and Gedit rename first the existing file
                    // and then write a new one with the same name.
                    // First we give same time to finish writing the new file.
                    sleep(Duration::from_millis(WAIT_EDITOR_WRITING_NEW_FILE));
                    // Then we have to set up the watcher again.
                    self.watcher
                        .watch(path.clone(), RecursiveMode::NonRecursive)
                        .map(|_| Self::update(&self.event_tx_list))?
                }

                // These we can ignore.
                DebouncedEvent::NoticeWrite(_) | DebouncedEvent::Rescan => {}

                // This is what most text editors do, the modify the existing file.
                DebouncedEvent::Write(_) | DebouncedEvent::Chmod(_) | DebouncedEvent::Create(_) =>
                // Inform web clients.
                {
                    Self::update(&self.event_tx_list)
                }

                // Here we better restart the whole watcher again, if possible. Seems fatal.
                DebouncedEvent::Rename(_path, _) => return Err(ViewerError::LostRenamedFile),

                // Dito.
                DebouncedEvent::Error(err, _path) => return Err(err.into()),
            }
        }
    }

    /// Run sub-command and notify subscribers.
    pub fn update(event_tx_list: &Arc<Mutex<Vec<Sender<()>>>>) {
        // Notify subscribers and forget disconnected subscribers.
        let tx_list = &mut *event_tx_list.lock().unwrap();
        *tx_list = tx_list.drain(..).filter(|tx| tx.send(()).is_ok()).collect();

        log::debug!(
            "FileWatcher::update(): {} subscribers updated.",
            tx_list.len()
        );
    }
}
