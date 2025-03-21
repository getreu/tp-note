//! Receives strings by a message channel, queues them and displays them
//! one by one in popup alert windows.

use std::sync::mpsc::sync_channel;
use std::sync::mpsc::Receiver;
use std::sync::mpsc::RecvTimeoutError;
use std::sync::mpsc::SendError;
use std::sync::mpsc::SyncSender;
use std::sync::LazyLock;
use std::sync::Mutex;
use std::thread;
use std::thread::sleep;
use std::time::Duration;

/// The number of messages that will be queued.
/// As error messages can drop in by every thread and we can only
/// show one alert window at the same time, they must be queued.
pub const QUEUE_LEN: usize = 30;

/// The `AlertService` reports to be busy as long as there is a message window
/// open and beyond that also `KEEP_ALIVE` milliseconds after the last message
/// window got closed by the user.
#[cfg(feature = "message-box")]
const KEEP_ALIVE: u64 = 1000;

/// Extra timeout for the `flush()` method, before it checks if there is still
/// an open popup alert window. We wait a moment just in case that there are
/// pending messages we have not received yet. 1 millisecond is enough, we wait
/// 10 just to be sure.
const FLUSH_TIMEOUT: u64 = 10;

/// Hold `AlertService` in a static variable, that
/// `AlertService::push_str()` can be called easily from everywhere.
static ALERT_SERVICE: LazyLock<AlertService> = LazyLock::new(|| {
    AlertService {
        // The message queue accepting strings for being shown as
        // popup alert windows.
        message_channel: {
            let (tx, rx) = sync_channel(QUEUE_LEN);
            (tx, Mutex::new(rx))
        },
        // This mutex does not hold any data. When it is locked, it indicates,
        // that the `AlertService` is still busy and should not get shut down.
        busy_lock: Mutex::new(()),
        // We start with no function pointer.
        popup_alert: Mutex::new(None),
    }
});

pub struct AlertService {
    /// The message queue accepting strings for being shown as
    /// popup alert windows.
    message_channel: (SyncSender<String>, Mutex<Receiver<String>>),
    /// This mutex does not hold any data. When it is locked, it indicates,
    /// that the `AlertService` is still busy and should not get shut down.
    busy_lock: Mutex<()>,
    /// Function pointer to the function that is called when the
    /// popup alert dialog shall appear.
    /// None means no function pointer was registered.
    popup_alert: Mutex<Option<fn(&str)>>,
}

impl AlertService {
    /// Initializes the service. Call once when the application starts.
    /// Drop strings in the`ALERT_SERVICE.message_channel` to use this service.
    pub fn init(popup_alert: fn(&str)) {
        // Setup the `AlertService`.
        // Set up the channel now.
        LazyLock::force(&ALERT_SERVICE);
        *ALERT_SERVICE.popup_alert.lock().unwrap() = Some(popup_alert);
        thread::spawn(move || {
            // This will block until the previous message has been received.
            AlertService::run();
        });
    }

    /// Alert service, receiving Strings to display in a popup window.
    fn run() {
        // Get the receiver.
        let (_, rx) = &ALERT_SERVICE.message_channel;
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
                        opt_guard = ALERT_SERVICE.busy_lock.try_lock().ok();
                    }
                    match *ALERT_SERVICE.popup_alert.lock().unwrap() {
                        // This blocks until the user closes the alert window.
                        Some(popup_alert) => popup_alert(&s),
                        _ => panic!(
                            "Can not print message \"{}\". \
                            No alert function registered!",
                            &s
                        ),
                    };
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
        let _res = ALERT_SERVICE.busy_lock.lock();
    }

    #[inline]
    /// Pushes `msg` into queue. In case the message queue is full, the method
    /// blocks until there is more free space. Make sure to initialize before
    /// with `AlertService::init()` Returns an `SendError` if nobody listens on
    /// `rx` of the queue. This can happen, e.g. if `AlertService::init()` has
    /// not been called before.
    pub fn push_str(msg: String) -> Result<(), SendError<String>> {
        let (tx, _) = &ALERT_SERVICE.message_channel;
        tx.send(msg)
    }
}
