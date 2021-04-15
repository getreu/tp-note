//! Prints error messages and exceptional states.

use crate::config::ARGS;
use crate::config::CONFIG_PATH;
#[cfg(feature = "message-box")]
use crate::config::RUNS_ON_CONSOLE;
#[cfg(feature = "message-box")]
use crate::VERSION;
#[cfg(feature = "message-box")]
use lazy_static::lazy_static;
use log::LevelFilter;
use log::{Level, Metadata, Record};
#[cfg(feature = "message-box")]
use msgbox::IconType;
use std::env;
use std::path::PathBuf;
#[cfg(feature = "message-box")]
use std::sync::mpsc::sync_channel;
#[cfg(feature = "message-box")]
use std::sync::mpsc::Receiver;
#[cfg(feature = "message-box")]
use std::sync::mpsc::RecvTimeoutError;
#[cfg(feature = "message-box")]
use std::sync::mpsc::SyncSender;
#[cfg(feature = "message-box")]
use std::sync::Mutex;
#[cfg(feature = "message-box")]
use std::thread;
#[cfg(feature = "message-box")]
use std::time::Duration;

/// As error messages can drop in by every thread, they must be queued.
/// The number of error messages that will be queued,
#[cfg(feature = "message-box")]
pub const ALERT_SERVICE_QUEUE_LEN: usize = 30;

/// The `AlertService` reports to be busy as long as there
/// is is a message window open and beyond that also
/// `ALERT_SERVICE_KEEP_ALIVE` milliseconds after the last
/// message window got closed by the user.
#[cfg(feature = "message-box")]
pub const ALERT_SERVICE_KEEP_ALIVE: u64 = 1000;

/// Window title of the message alert box.
#[cfg(feature = "message-box")]
const ALERT_DIALOG_TITLE: &str = "Tp-Note";

////////////////////////////
/// AppLogger
////////////////////////////

pub struct AppLogger;
pub static APP_LOGGER: AppLogger = AppLogger;

/// Initialize logger.
impl AppLogger {
    pub fn init() {
        // Setup `AlertService`.
        #[cfg(feature = "message-box")]
        if !*RUNS_ON_CONSOLE && !ARGS.batch {
            // Set up the channel now.
            lazy_static::initialize(&ALERT_SERVICE_CHANNEL);
            thread::spawn(move || {
                // this will block until the previous message has been received
                AlertService::run();
            });
        };

        // Setup console logger.
        log::set_logger(&APP_LOGGER).unwrap();
        if let Some(level) = ARGS.debug {
            log::set_max_level(level);
        } else {
            log::set_max_level(LevelFilter::Error);
        }
    }

    /// Blocks until the `AlertService` is not busy any more.
    /// This should be executed before quitting the application
    /// because there might be still queued error messages
    /// the uses has not seen yet.
    pub fn wait_when_busy() {
        // If ever there is still a message window open, this will block.
        #[cfg(feature = "message-box")]
        AlertService::wait_when_busy();
    }

    /// Adds a footer with additional debugging information, such as
    /// command line parameters and configuration file path.
    fn format_error(msg: &str) -> String {
        // Remember the command-line-arguments.
        let mut args_str = String::new();
        for argument in env::args() {
            args_str.push_str(argument.as_str());
            args_str.push(' ');
        }

        let mut s = String::from(msg);
        s.push_str(&format!(
            "\n\
            ---\n\
            Additional technical details:\n\
            *    Command line parameters:\n\
            {}\n\
            *    Configuration file:\n\
            {}",
            args_str,
            &*CONFIG_PATH
                .as_ref()
                .unwrap_or(&PathBuf::from("no path found"))
                .to_str()
                .unwrap_or_default()
        ));
        s
    }
}

/// Trait defining the logging format and destination.
impl log::Log for AppLogger {
    fn enabled(&self, metadata: &Metadata) -> bool {
        metadata.level() <= Level::Trace
    }

    fn log(&self, record: &Record) {
        if self.enabled(record.metadata()) {
            if (record.metadata().level() == Level::Error) || ARGS.popup {
                let msg = if record.metadata().level() == Level::Error {
                    format!(
                        "{}:\n{}",
                        record.level(),
                        &Self::format_error(&record.args().to_string())
                    )
                } else {
                    format!("{}:\n{}", record.level(), &record.args().to_string())
                };
                eprintln!("*** {}", msg);

                #[cfg(feature = "message-box")]
                if !*RUNS_ON_CONSOLE && !ARGS.batch {
                    let (tx, _) = &*ALERT_SERVICE_CHANNEL;
                    let tx = tx.clone();
                    tx.send(msg).unwrap();
                }
            } else {
                eprintln!("*** {}: {}", record.level(), record.args());
            }
        }
    }
    fn flush(&self) {}
}

////////////////////////////
// AlertService
////////////////////////////

#[cfg(feature = "message-box")]
lazy_static! {
/// This is the message queue from `AppLogger` to `AlertService`.
    pub static ref ALERT_SERVICE_CHANNEL: (SyncSender<String>, Mutex<Receiver<String>>) = {
        let (tx, rx) = sync_channel(ALERT_SERVICE_QUEUE_LEN);
        (tx, Mutex::new(rx))
    };
}

#[cfg(feature = "message-box")]
lazy_static! {
    /// Window title followed by version.
    pub static ref ALERT_DIALOG_TITLE_LINE: String = format!(
        "{} (v{})",
        &ALERT_DIALOG_TITLE,
        VERSION.unwrap_or("unknown")
    );
}

#[cfg(feature = "message-box")]
lazy_static! {
    /// This mutex does not hold any data. When it is locked, it indicates,
    /// that the `AlertService` is still busy and should not get shut down.
    static ref ALERT_SERVICE_BUSY: Mutex<()> = Mutex::new(());
}

#[cfg(feature = "message-box")]
pub struct AlertService {}

#[cfg(feature = "message-box")]
impl AlertService {
    /// Alert service, receiving Strings to display in a popup window.
    fn run() {
        // Get the receiver.
        let (_, rx) = &*ALERT_SERVICE_CHANNEL;
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
                // If the next `ALERT_SERVICE_KEEP_ALIVE` milliseconds no
                // other message comes in, we release the lock again.
                match rx.recv_timeout(Duration::from_millis(ALERT_SERVICE_KEEP_ALIVE)) {
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
                        opt_guard = ALERT_SERVICE_BUSY.try_lock().ok();
                    }
                    // This blocks until the user closes the alert window.
                    Self::print_error(&s);
                }
                // `ALERT_SERVICE_KEEP_ALIVE` milliseconds are over and still no new message.
                // We release the lock again.
                None => {
                    // Here the `guard` goes out of scope and the lock is released.
                    opt_guard = None;
                    //
                }
            }
        }
    }

    /// The `AlertService` keeps holding a lock until `ALERT_SERVICE_KEEP_ALIVE` milliseconds after
    /// the user has closed that last error message. Only then it releases the lock. This function
    /// blocks until the lock is released.
    fn wait_when_busy() {
        // This might block, if a guard in `run()` holds already a lock.
        let _ = ALERT_SERVICE_BUSY.lock();
    }

    /// Pops up an error message box and prints `msg`.
    /// Blocks until the user closes the window.
    fn print_error(msg: &str) {
        let _ = msgbox::create(&*ALERT_DIALOG_TITLE_LINE, &msg, IconType::Info);
    }
}
