//! Implements the file watcher for the note viewer feature.

extern crate notify;

use crate::config::CFG;
use crate::viewer::error::ViewerError;
use notify::{watcher, DebouncedEvent, RecommendedWatcher, RecursiveMode, Watcher};
use std::panic::panic_any;
use std::path::PathBuf;
use std::sync::mpsc::{channel, Receiver, RecvTimeoutError, Sender};
use std::sync::{Arc, Mutex};
use std::thread::sleep;
use std::time::Duration;
use std::time::Instant;

/// Some file editors do not modify the file on disk, they move the old version
/// away and write a new file with the same name. As this takes some time,
/// the watcher waits a bit before trying to access the new file.
/// The delay is in milliseconds.
const WAIT_EDITOR_WRITING_NEW_FILE: u64 = 200;

/// Delay while `update()` with no subscribers silently ignored. This avoids
/// a race condition, when a file has already changed on disk, but the browser
/// has not connected yet.
const WATCHER_MIN_UPTIME: u128 = 3000;

#[derive(Debug)]
/// Object describing how `State` transitions will happen.  State changes can
/// take place only each tick.
pub enum Mode {
    /// Next state is `State::Started`, the following state will be `State::Blocking`.
    OneTick(u64),
    /// Next state is `State::Ticking`.
    Ticks(u64),
    /// Next state is `State::Blocking.
    #[allow(dead_code)]
    Blocking,
}

/// State of the watcher. It determines if and when extra `update()` events will
/// be sent to the web browser in order to check if it is still connected.
#[derive(Debug)]
pub enum State {
    /// Starting state. After next tick goes into `Blocking`
    Started(u64),
    /// Send periodic extra `update()` events to the web browser.
    /// The parameter is the time interval in seconds between ticks.
    /// If `Mode` does not change, the state remains `Ticking`.
    Ticking(u64),
    /// In `Blocking` state, `update()` events are only sent to the web browser
    /// when the observed file changes. No extra `update()` events are sent.
    /// If `Mode` does not change, the state remains `Blocking`.
    Blocking,
}

/// The `watcher` notifies about changes through `rx`.
pub struct FileWatcher<State> {
    /// Receiver for file changed messages.
    rx: Receiver<DebouncedEvent>,
    /// File watcher.
    watcher: RecommendedWatcher,
    /// List of subscribers to inform when the file is changed.
    event_tx_list: Arc<Mutex<Vec<Sender<()>>>>,
    /// `Mode` object describing how `state` transitions happen.
    mode: Arc<Mutex<Mode>>,
    /// State.
    state: State,
    /// StartTime of this file-watcher.
    start_time: Instant,
}

/// Watch file changes and notify subscribers.
impl FileWatcher<State> {
    /// Constructor. `file` is the file to watch.
    pub fn new(
        file: PathBuf,
        event_tx_list: Arc<Mutex<Vec<Sender<()>>>>,
        mode: Arc<Mutex<Mode>>,
    ) -> Result<Self, ViewerError> {
        let notify_period = CFG.viewer_notify_period;
        let (tx, rx) = channel();
        let mut watcher = watcher(tx, Duration::from_millis(notify_period))?;
        watcher.watch(&file, RecursiveMode::Recursive)?;

        let state = match *mode.lock().unwrap() {
            Mode::OneTick(interval) => State::Started(interval),
            Mode::Ticks(interval) => State::Ticking(interval),
            Mode::Blocking => State::Blocking,
        };

        log::debug!(
            "File watcher started (mode: {:?}, state: {:?})",
            *mode.lock().unwrap(),
            state
        );

        Ok(Self {
            rx,
            watcher,
            event_tx_list,
            // By default, tick only once.
            mode,
            state,
            start_time: Instant::now(),
        })
    }

    /// Wrapper to start the server.
    pub fn run(&mut self) {
        match Self::run2(self) {
            Ok(_) => (),
            Err(e) => {
                log::debug!("File watcher terminated: {}", e);
            }
        }
    }

    /// Start the file watcher. Blocks forever, unless an `ViewerError::AllSubscriberDiconnected`
    /// occurs.
    fn run2(&mut self) -> Result<(), ViewerError> {
        loop {
            // Wait for file modifications.
            let evnt = match self.state {
                State::Started(interval) | State::Ticking(interval) => {
                    match self.rx.recv_timeout(Duration::from_secs(interval)) {
                        Ok(ev) => ev,
                        Err(RecvTimeoutError::Timeout) => {
                            // State transition:
                            self.state = match *self.mode.lock().unwrap() {
                                Mode::OneTick(_) => State::Blocking,
                                Mode::Ticks(interval) => State::Ticking(interval),
                                Mode::Blocking => State::Blocking,
                            };
                            // Send subscriber an update event in order to check if they
                            // are still connected.
                            self.update()?;
                            continue;
                        }
                        // The sending half of a channel (or sync_channel) is `Disconnected`,
                        // implies that no further messages will ever be received.
                        // As this should never happen, we panic this thread then.
                        Err(RecvTimeoutError::Disconnected) => panic_any(()),
                    }
                }
                State::Blocking => self.rx.recv().unwrap(),
            };

            log::trace!("File watcher ({:?}) event: {:?}", self.state, evnt);

            match evnt {
                DebouncedEvent::NoticeRemove(path) | DebouncedEvent::Remove(path) => {
                    // Some text editors e.g. Neovim and Gedit rename first the existing file
                    // and then write a new one with the same name.
                    // First we give same time to finish writing the new file.
                    sleep(Duration::from_millis(WAIT_EDITOR_WRITING_NEW_FILE));
                    // Then we have to set up the watcher again.
                    self.watcher
                        .watch(path.clone(), RecursiveMode::NonRecursive)
                        .map_err(|e| e.into())
                        .and_then(|_| self.update())?
                }

                // These we can ignore.
                DebouncedEvent::NoticeWrite(_) | DebouncedEvent::Rescan => {}

                // This is what most text editors do, the modify the existing file.
                DebouncedEvent::Write(_) | DebouncedEvent::Chmod(_) | DebouncedEvent::Create(_) =>
                // Inform web clients.
                {
                    self.update()?
                }

                // Here we better restart the whole watcher again, if possible. Seems fatal.
                DebouncedEvent::Rename(_path, _) => return Err(ViewerError::LostRenamedFile),

                // Dito.
                DebouncedEvent::Error(err, _path) => return Err(err.into()),
            }
        }
    }

    /// Run sub-command and notify subscribers.
    pub fn update(&self) -> Result<(), ViewerError> {
        // Notify subscribers and forget disconnected subscribers.
        let tx_list = &mut *self.event_tx_list.lock().unwrap();
        *tx_list = tx_list.drain(..).filter(|tx| tx.send(()).is_ok()).collect();

        log::debug!(
            "File watcher (mode: {:?}, state: {:?}) `update()`: {} subscribers updated.",
            *self.mode.lock().unwrap(),
            self.state,
            tx_list.len()
        );

        // When empty all subscribers have disconnected.
        if tx_list.is_empty() && self.start_time.elapsed().as_millis() > WATCHER_MIN_UPTIME {
            return Err(ViewerError::AllSubscriberDiconnected);
        }
        Ok(())
    }
}
