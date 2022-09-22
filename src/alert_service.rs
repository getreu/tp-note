//! Receives strings by a message channel, queues them and displays them
//! one by one in popup alert windows.

use crate::logger::ALERT_DIALOG_TITLE_LINE;
use lazy_static::lazy_static;
use msgbox::IconType;
use std::sync::mpsc::sync_channel;
use std::sync::mpsc::Receiver;
use std::sync::mpsc::RecvTimeoutError;
use std::sync::mpsc::SendError;
use std::sync::mpsc::SyncSender;
use std::sync::Mutex;
use std::thread;
use std::thread::sleep;
use std::time::Duration;

lazy_static! {
/// The message queue accepting strings for being shown as
/// popup alert windows.
    static ref MESSAGE_CHANNEL: (SyncSender<String>, Mutex<Receiver<String>>) = {
        let (tx, rx) = sync_channel(QUEUE_LEN);
        (tx, Mutex::new(rx))
    };
}

lazy_static! {
    /// This mutex does not hold any data. When it is locked, it indicates,
    /// that the `AlertService` is still busy and should not get shut down.
    static ref BUSY_LOCK: Mutex<()> = Mutex::new(());
}

/// The number of messages that will be queued.
/// As error messages can drop in by every thread and we can only
/// show one alert window at the same time, they must be queued.
pub const QUEUE_LEN: usize = 30;

/// The `AlertService` reports to be busy as long as there
/// is is a message window open and beyond that also
/// `KEEP_ALIVE` milliseconds after the last
/// message window got closed by the user.
#[cfg(feature = "message-box")]
const KEEP_ALIVE: u64 = 1000;

/// Extra timeout for the `flush()` method, before it checks if there is still
/// an open popup alert window.  We wait a moment just in case that there are
/// pending messages we have not received yet. 1 millisecond is enough, we wait
/// 10 just to be sure.
const FLUSH_TIMEOUT: u64 = 10;

pub struct AlertService {}

impl AlertService {
    /// Initializes the service. Call once when the application starts.
    /// Drop strings in the`MESSAGE_CHANNEL` to use this service.
    pub fn init() {
        // Setup the `AlertService`.
        // Set up the channel now.
        lazy_static::initialize(&MESSAGE_CHANNEL);
        thread::spawn(move || {
            // this will block until the previous message has been received
            AlertService::run();
        });
    }

    /// Alert service, receiving Strings to display in a popup window.
    fn run() {
        // Get the receiver.
        let (_, rx) = &*MESSAGE_CHANNEL;
        let rx = rx.lock().unwrap();

        // We start with the lock released.
        let mut opt_guard = None;
        loop {
            let msg = if opt_guard.is_none() {
                // As there is no lock, we block here until the next message comes.
                // `recv()` should never return `Err`. This can only happen when
                // the sending half of a channel (or sync_channel) is disconnected,
                // implying that no further messages will ever be received.
                // As this should never happen, we panic this thread then.
                Some(rx.recv().unwrap())
            } else {
                // There is a lock because we just received another message.
                // If the next `KEEP_ALIVE` milliseconds no
                // other message comes in, we release the lock again.
                match rx.recv_timeout(Duration::from_millis(KEEP_ALIVE)) {
                    Ok(s) => Some(s),
                    Err(RecvTimeoutError::Timeout) => None,
                    // The sending half of a channel (or sync_channel) is `Disconnected`,
                    // implies that no further messages will ever be received.
                    // As this should never happen, we panic this thread then.
                    Err(RecvTimeoutError::Disconnected) => panic!(),
                }
            };

            // We received a message.
            match msg {
                Some(s) => {
                    // If the lock is released, lock it now.
                    if opt_guard.is_none() {
                        opt_guard = BUSY_LOCK.try_lock().ok();
                    }
                    // This blocks until the user closes the alert window.
                    Self::print_error(&s);
                }
                // `ALERT_SERVICE_KEEP_ALIVE` milliseconds are over and still no
                // new message. We release the lock again.
                None => {
                    // Here the `guard` goes out of scope and the lock is released.
                    opt_guard = None;
                    //
                }
            }
        }
    }

    /// The `AlertService` keeps holding a lock until `KEEP_ALIVE` milliseconds
    /// after the user has closed that last error alert window. Only then, it
    /// releases the lock. This function blocks until the lock is released.
    pub fn flush() {
        // See constant documentation why we wait here.
        sleep(Duration::from_millis(FLUSH_TIMEOUT));
        // This might block, if a guard in `run()` holds already a lock.
        let _res = BUSY_LOCK.lock();
    }

    #[inline]
    /// Pushes `msg` into queue. In case the message queue is full, the method
    /// blocks until there is more free space. Make sure to initialize before
    /// with `AlertService::init()` Returns an `SendError` if nobody listens on
    /// `rx` of the queue. This can happen, e.g. if `AlertService::init()` has
    /// not been called before.
    pub fn push_str(msg: String) -> Result<(), SendError<String>> {
        let (tx, _) = &*MESSAGE_CHANNEL;
        tx.send(msg)
    }

    #[inline]
    /// Pops up an error message box and prints `msg`.
    /// Blocks until the user closes the window.
    fn print_error(msg: &str) {
        let _ = msgbox::create(&*ALERT_DIALOG_TITLE_LINE, msg, IconType::Info);
    }
}
