//! Implements the file watcher for the note viewer feature.

use crate::config::CFG;
use crate::viewer::error::ViewerError;
use crate::viewer::sse_server::SseToken;
use notify::{RecommendedWatcher, RecursiveMode};
use notify_debouncer_mini::{new_debouncer, DebouncedEvent, Debouncer};
use std::panic::panic_any;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::TrySendError;
use std::sync::mpsc::{channel, Receiver, RecvTimeoutError, SyncSender};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use std::time::Instant;

/// Even if there is no file modification, after `WATCHER_TIMEOUT` seconds,
/// the watcher sends an `update` request to check if there are still
/// subscribers connected.
const WATCHER_TIMEOUT: u64 = 10;

/// Delay while `update()` with no subscribers silently ignored. This avoids
/// a race condition, when a file has already changed on disk, but the browser
/// has not connected yet.
const WATCHER_MIN_UPTIME: u128 = 3000;
/// The `watcher` notifies about changes through `rx`.
pub struct FileWatcher {
    /// Receiver for file changed messages.
    rx: Receiver<Result<Vec<DebouncedEvent>, Vec<notify::Error>>>,
    /// We must store the `Debouncer` because it hold
    /// the sender of the channel.
    #[allow(dead_code)]
    debouncer: Debouncer<RecommendedWatcher>,
    /// List of subscribers to inform when the file is changed.
    event_tx_list: Arc<Mutex<Vec<SyncSender<SseToken>>>>,
    /// Send additional periodic update events to detect when
    /// the browser disconnects.
    terminate_on_browser_disconnect: Arc<AtomicBool>,
    /// Start time of this file-watcher.
    start_time: Instant,
}

/// Watch file changes and notify subscribers.
impl FileWatcher {
    /// Constructor. `file` is the file to watch.
    pub fn new(
        file: PathBuf,
        event_tx_list: Arc<Mutex<Vec<SyncSender<SseToken>>>>,
        terminate_on_browser_disconnect: Arc<AtomicBool>,
    ) -> Result<Self, ViewerError> {
        let notify_period = CFG.viewer.notify_period;
        let (tx, rx) = channel();
        // Max value for `notify_period` is 2 seconds.
        let mut debouncer = new_debouncer(Duration::from_millis(notify_period), None, tx)?;
        debouncer.watcher().watch(&file, RecursiveMode::Recursive)?;

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
                    if tx_list.is_empty()
                        && self.start_time.elapsed().as_millis() > WATCHER_MIN_UPTIME
                        && self.terminate_on_browser_disconnect.load(Ordering::SeqCst)
                    {
                        return Err(ViewerError::AllSubscriberDiconnected);
                    }
                    continue;
                }
                // The  sending half of a channel (or sync_channel) is `Disconnected`,
                // implies that no further messages will ever be received.
                // As this should never happen, we panic this thread then.
                Err(RecvTimeoutError::Disconnected) => panic_any("RecvTimeoutError::Disconnected"),
            };

            log::trace!("File watcher event: {:?}", evnt);

            match evnt {
                Ok(_) => self.update(SseToken::Update)?,
                Err(mut e) => return Err(e.remove(1).into()),
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
