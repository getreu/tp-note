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
use std::sync::RwLock;
#[cfg(feature = "message-box")]
use std::thread;
#[cfg(feature = "message-box")]
use std::time::Duration;

/// The number of messages that will be queued.
/// As error messages can drop in by every thread and we can only
/// show one alert window at the same time, they must be queued.
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
// AppLogger
////////////////////////////

lazy_static! {
    /// If `true`, all future log events will trigger the opening of a popup
    /// alert window. Otherwise only `Level::Error` will do.
    static ref APP_LOGGER_POPUP_ALWAYS_ENABLED: RwLock<bool> = RwLock::new(false);
}

pub struct AppLogger;
pub static APP_LOGGER: AppLogger = AppLogger;

/// Initialize logger.
impl AppLogger {
    #[inline]
    pub fn init() {
        // Setup the `AlertService`
        #[cfg(feature = "message-box")]
        AlertService::init();

        // Setup console logger.
        log::set_logger(&APP_LOGGER).unwrap();
        log::set_max_level(LevelFilter::Error);
    }

    /// Sets the maximum level debug events must have to be logged.
    #[allow(dead_code)]
    pub fn set_max_level(level: LevelFilter) {
        log::set_max_level(level);
    }

    /// If called with `true`, all debug events will also trigger the appearance of
    /// a popup alert window.
    #[allow(dead_code)]
    pub fn set_popup_always_enabled(popup: bool) {
        // This blocks if ever another thread wants to write.  As we are the only ones to write
        // here, this lock can never get poisoned and we will can safely `unwrap()` here.
        let mut lock = APP_LOGGER_POPUP_ALWAYS_ENABLED.write().unwrap();
        *lock = popup;
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
            // Log this to `stderr`.
            eprintln!("*** {}: {}", record.level(), record.args());

            // Eventually also log as popup alert window.
            #[cfg(feature = "message-box")]
            if !*RUNS_ON_CONSOLE
                && !ARGS.batch
                && ((record.metadata().level() == LevelFilter::Error)
                        // This lock can never get poisoned, so `unwrap()` is safe here.
                        || *(APP_LOGGER_POPUP_ALWAYS_ENABLED.read().unwrap()))
            {
                let msg = if record.metadata().level() == Level::Error {
                    format!(
                        "{}:\n{}",
                        record.level(),
                        &Self::format_error(&record.args().to_string())
                    )
                } else {
                    format!("{}:\n{}", record.level(), &record.args().to_string())
                };

                let (tx, _) = &*ALERT_SERVICE_CHANNEL;
                let tx = tx.clone();
                tx.send(msg).unwrap();
            };
        }
    }

    fn flush(&self) {}
}

////////////////////////////
// AlertService
////////////////////////////

#[cfg(feature = "message-box")]
lazy_static! {
/// The message queue accepting strings for being shown as
/// popup alert windows.
    pub static ref ALERT_SERVICE_CHANNEL: (SyncSender<String>, Mutex<Receiver<String>>) = {
        let (tx, rx) = sync_channel(ALERT_SERVICE_QUEUE_LEN);
        (tx, Mutex::new(rx))
    };
}

#[cfg(feature = "message-box")]
lazy_static! {
    /// Window title followed by version.
    static ref ALERT_DIALOG_TITLE_LINE: String = format!(
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
    /// Initializes the service. Call once when the application starts.
    /// Drop strings in the`ALERT_SERVICE_CHANNEL` to use this service.
    pub fn init() {
        // Setup the `AlertService`.
        #[cfg(feature = "message-box")]
        if !*RUNS_ON_CONSOLE && !ARGS.batch {
            // Set up the channel now.
            lazy_static::initialize(&ALERT_SERVICE_CHANNEL);
            thread::spawn(move || {
                // this will block until the previous message has been received
                AlertService::run();
            });
        };
    }

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
    /// the user has closed that last error message. Only then, it releases the lock. This function
    /// blocks until the lock is released.
    pub fn wait_when_busy() {
        // This might block, if a guard in `run()` holds already a lock.
        let _ = ALERT_SERVICE_BUSY.lock();
    }

    /// Pops up an error message box and prints `msg`.
    /// Blocks until the user closes the window.
    fn print_error(msg: &str) {
        let _ = msgbox::create(&*ALERT_DIALOG_TITLE_LINE, &msg, IconType::Info);
    }
}
