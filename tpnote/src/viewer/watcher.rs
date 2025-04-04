//! Implements the file watcher for the note viewer feature.

use crate::config::CFG;
use crate::viewer::error::ViewerError;
use crate::viewer::sse_server::SseToken;
use notify::RecursiveMode;
use notify_debouncer_mini::Config;
use notify_debouncer_mini::{new_debouncer_opt, DebouncedEvent, Debouncer};
use std::panic::panic_any;
use std::path::Path;
use std::sync::mpsc::TrySendError;
use std::sync::mpsc::{channel, Receiver, RecvTimeoutError, SyncSender};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use std::time::Instant;

/// Even if there is no file modification, after `WATCHER_TIMEOUT` seconds,
/// the watcher sends an `update` request to the connected web browsers in
/// order to check if there are still subscribers connected. The value's unit
/// is seconds.
const WATCHER_TIMEOUT: u64 = 10;

/// Delay while `update()` with no subscribers silently ignored. This avoids
/// a race condition, when a file has already changed on disk, but the browser
/// has not connected yet. The value's unit is seconds.
const WATCHER_MIN_UPTIME: u64 = 5;
/// The `watcher` notifies about changes through `rx`.
pub struct FileWatcher {
    /// Receiver for file changed messages.
    rx: Receiver<Result<Vec<DebouncedEvent>, notify::Error>>,
    /// We must store the `Debouncer` because it hold
    /// the sender of the channel.
    #[allow(dead_code)]
    debouncer: Debouncer<notify::PollWatcher>,
    /// List of subscribers to inform when the file is changed.
    event_tx_list: Arc<Mutex<Vec<SyncSender<SseToken>>>>,
    /// Send additional periodic update events to detect when
    /// the browser disconnects.
    terminate_on_browser_disconnect: Arc<Mutex<bool>>,
    /// Start time of this file-watcher.
    start_time: Instant,
}

/// Watch file changes and notify subscribers.
impl FileWatcher {
    /// Constructor. `file` is the file to watch.
    pub fn new(
        // The file path of the file being watched.
        watched_file: &Path,
        // A list of subscribers, that shall be informed when the watched
        // file has been changed.
        event_tx_list: Arc<Mutex<Vec<SyncSender<SseToken>>>>,
        terminate_on_browser_disconnect: Arc<Mutex<bool>>,
    ) -> Result<Self, ViewerError> {
        let (tx, rx) = channel();
        // Max value for `notify_period` is 2 seconds.
        // We use the same value for `timeout` and `Some(tick_rate)`.
        let notify_period = Duration::from_millis(CFG.viewer.notify_period);
        let backend_config = notify::Config::default().with_poll_interval(notify_period);
        // Debouncer configuration
        let debouncer_config = Config::default()
            .with_timeout(notify_period)
            .with_notify_config(backend_config);
        // Select backend via fish operator, here PollWatcher backend
        let mut debouncer = new_debouncer_opt::<_, notify::PollWatcher>(debouncer_config, tx)?;
        // In theory watching only `file` is enough. Unfortunately some file
        // editors do not modify files directly. They first rename the existing
        // file on disk and then create a new file with the same filename.
        // Older versions of Notify did not detect this case reliably.
        debouncer
            .watcher()
            .watch(watched_file, RecursiveMode::NonRecursive)?;

        log::debug!("File watcher started.");

        Ok(Self {
            rx,
            debouncer,
            event_tx_list,
            start_time: Instant::now(),
            terminate_on_browser_disconnect,
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

    /// Start the file watcher. Blocks forever, unless an `ViewerError::AllSubscriberDisconnected`
    /// occurs.
    fn run2(&mut self) -> Result<(), ViewerError> {
        loop {
            // Detect when the browser quits, then terminate the watcher.
            let evnt = match self.rx.recv_timeout(Duration::from_secs(WATCHER_TIMEOUT)) {
                Ok(ev) => ev,
                Err(RecvTimeoutError::Timeout) => {
                    // Push something to detect disconnected TCP channels.
                    self.update(SseToken::Ping)?;

                    // When empty all TCP connections have disconnected.
                    let tx_list = &mut *self.event_tx_list.lock().unwrap();
                    // log::trace!(
                    //     "File watcher timeout: {} open TCP connections.",
                    //     tx_list.len()
                    // );
                    {
                        if tx_list.is_empty()
                            && self.start_time.elapsed().as_secs() > WATCHER_MIN_UPTIME
                            // Release lock immediately.
                            && *self.terminate_on_browser_disconnect.lock().unwrap()
                        {
                            return Err(ViewerError::AllSubscriberDiconnected);
                        }
                    }
                    continue;
                }
                // The sending half of a channel (or sync_channel) is
                // `Disconnected`, implies that no further messages will ever be
                // received. As this should never happen, we panic this thread
                // then.
                Err(RecvTimeoutError::Disconnected) => panic_any("RecvTimeoutError::Disconnected"),
            };

            log::trace!("File watcher event: {:?}", evnt);

            match evnt {
                Ok(_events) => {
                    // There can be more than one event in `event`, we
                    // don't care about the details as we watch only one
                    // file.
                    self.update(SseToken::Update)?;
                }
                Err(e) => return Err(e.into()),
            }
        }
    }

    /// Run sub-command and notify subscribers.
    pub fn update(&self, msg: SseToken) -> Result<(), ViewerError> {
        // Notify subscribers and forget disconnected subscribers.
        let tx_list = &mut *self.event_tx_list.lock().unwrap();
        let tx_list_len_before_update = tx_list.len();
        *tx_list = tx_list
            .drain(..)
            .filter(|tx| match tx.try_send(msg.to_owned()) {
                Ok(()) => true,
                Err(TrySendError::Disconnected(_)) => false,
                Err(_) => true,
            })
            .collect();
        let tx_list_len = tx_list.len();
        log::trace!(
            "File watcher `update({:?})`: {} dropped TCP connections, {} still open.",
            msg,
            tx_list_len_before_update - tx_list_len,
            tx_list_len,
        );

        Ok(())
    }
}
