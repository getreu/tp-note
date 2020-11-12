//! Implements the file watcher for the Markdown note viewer feature.

use crate::config::ARGS;
use crate::config::CFG;
use anyhow::anyhow;
use notify::{watcher, DebouncedEvent, RecommendedWatcher, RecursiveMode, Watcher};
use std::path::PathBuf;
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::time::Duration;

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
    pub fn new(file: PathBuf, event_tx_list: Arc<Mutex<Vec<Sender<()>>>>) -> Self {
        match Self::new2(file, event_tx_list) {
            Ok(fw) => fw,
            Err(e) => {
                panic!(format!("ERROR: Watcher::new(): {:?}", e));
            }
        }
    }

    /// Constructor. `file` is the file to watch.
    pub fn new2(
        file: PathBuf,
        event_tx_list: Arc<Mutex<Vec<Sender<()>>>>,
    ) -> Result<Self, anyhow::Error> {
        let notify_period = CFG.viewer_notify_period;
        let (tx, rx) = channel();
        let mut watcher = watcher(tx.clone(), Duration::from_millis(notify_period))?;
        watcher.watch(&file, RecursiveMode::Recursive)?;
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
                eprintln!("ERROR: Watcher::run(): {:?}", e);
            }
        }
    }

    /// Set up the file watcher and start the event/html server.
    fn run2(&mut self) -> Result<(), anyhow::Error> {
        loop {
            match self.rx.recv().unwrap() {
                // Ignore rescan and notices.
                DebouncedEvent::NoticeRemove(_)
                | DebouncedEvent::NoticeWrite(_)
                | DebouncedEvent::Rescan => {}

                // Actual modifications.
                DebouncedEvent::Write(_) | DebouncedEvent::Chmod(_) | DebouncedEvent::Create(_) => {
                    // Run the sub-command.
                    Self::update(&self.event_tx_list);
                }

                // Removal or replacement through renaming.
                DebouncedEvent::Remove(path) => {
                    // Instead of modifying the file, some weird editors
                    // (hello Gedit!) remove the current file and recreate it
                    // by renaming the buffer.
                    // To outsmart such editors, the watcher is set up to watch
                    // again a file with the same name. If this succeeds, the
                    // file is deemed changed.
                    self.watcher
                        .watch(path.clone(), RecursiveMode::NonRecursive)
                        .map_err(|e| anyhow!(e))
                        .and_then(|_| Ok(Self::update(&self.event_tx_list)))?
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
    pub fn update(event_tx_list: &Arc<Mutex<Vec<Sender<()>>>>) {
        // Notify subscribers and forget disconnected subscribers.
        let tx_list = &mut *event_tx_list.lock().unwrap();
        *tx_list = tx_list.drain(..).filter(|tx| tx.send(()).is_ok()).collect();
        if ARGS.debug {
            eprintln!(
                "*** Debug: Viewer::update(): {} subscribers updated.",
                tx_list.len()
            );
        };
    }
}
