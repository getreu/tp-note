//! Prints error messages and exceptional states.

#[cfg(feature = "message-box")]
use crate::alert_service::AlertService;
#[cfg(feature = "message-box")]
use crate::alert_service::MESSAGE_CHANNEL;
#[cfg(feature = "message-box")]
use crate::config::ARGS;
#[cfg(feature = "message-box")]
use crate::config::CONFIG_PATH;
#[cfg(feature = "message-box")]
use crate::config::RUNS_ON_CONSOLE;
use lazy_static::lazy_static;
use log::LevelFilter;
use log::{Level, Metadata, Record};
#[cfg(feature = "message-box")]
use std::env;
#[cfg(feature = "message-box")]
use std::path::PathBuf;
use std::sync::RwLock;

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
        #[cfg(feature = "message-box")]
        if !*RUNS_ON_CONSOLE && !ARGS.batch {
            // If ever there is still a message window open, this will block.
            AlertService::wait_when_busy();
        }
    }

    /// Adds a footer with additional debugging information, such as
    /// command line parameters and configuration file path.
    #[cfg(feature = "message-box")]
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

                let (tx, _) = &*MESSAGE_CHANNEL;
                let tx = tx.clone();
                tx.send(msg).unwrap();
            };
        }
    }

    fn flush(&self) {}
}
