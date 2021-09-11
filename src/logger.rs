//! Prints error messages and exceptional states.

#[cfg(feature = "message-box")]
use crate::alert_service::AlertService;
#[cfg(feature = "message-box")]
use crate::settings::ARGS;
#[cfg(feature = "message-box")]
use crate::settings::RUNS_ON_CONSOLE;
use lazy_static::lazy_static;
use log::LevelFilter;
use log::{Level, Metadata, Record};
use std::sync::atomic::{AtomicBool, Ordering};

pub struct AppLogger {
    /// If `true`, all future log events will trigger the opening of a popup
    /// alert window. Otherwise only `Level::Error` will do.
    popup_always_enabled: AtomicBool,
}

lazy_static! {
    static ref APP_LOGGER: AppLogger = AppLogger {
        popup_always_enabled: AtomicBool::new(false)
    };
}

/// All methods here are stateless (without _self_). Instead, their state is
/// stored in a global variable `APP_LOGGER` in order to simplify the API for
/// the caller.  As all the methods are stateless, the caller does not need to
/// carry around any (state) struct. For example, just `AppLogger::log(...)`
/// will do.
impl AppLogger {
    #[inline]
    /// Initialize logger.
    pub fn init() {
        // Setup the `AlertService`
        #[cfg(feature = "message-box")]
        if !*RUNS_ON_CONSOLE && !ARGS.batch {
            AlertService::init();
        };

        // Setup console logger.
        log::set_logger(&*APP_LOGGER).unwrap();
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
        // This blocks if ever another thread wants to write.  As we are the
        // only ones to write here, this lock can never get poisoned and we will
        // can safely `unwrap()` here.
        APP_LOGGER
            .popup_always_enabled
            .store(popup, Ordering::SeqCst);
    }

    /// Blocks until the `AlertService` is not busy any more. This should be
    /// executed before quitting the application because there might be still
    /// queued error messages the uses has not seen yet.
    pub fn flush() {
        #[cfg(feature = "message-box")]
        if !*RUNS_ON_CONSOLE && !ARGS.batch {
            // If ever there is still a message window open, this will block.
            AlertService::flush();
        }
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
                        || APP_LOGGER.popup_always_enabled.load(Ordering::SeqCst))
            {
                let msg = format!("{}:\n{}", record.level(), &record.args().to_string());
                // We silently ignore failing pushes. We have printed the
                // error message on the console already.
                let _res = if record.metadata().level() == Level::Error {
                    AlertService::push_debug_str(msg)
                } else {
                    AlertService::push_str(msg)
                };
            };
        }
    }

    fn flush(&self) {}
}
