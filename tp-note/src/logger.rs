//! Prints error messages and exceptional states.

#[cfg(feature = "message-box")]
use crate::alert_service::AlertService;
#[cfg(feature = "message-box")]
use crate::settings::ARGS;
#[cfg(feature = "message-box")]
use crate::settings::RUNS_ON_CONSOLE;
use crate::CONFIG_PATH;
#[cfg(feature = "message-box")]
use crate::VERSION;
use lazy_static::lazy_static;
use log::LevelFilter;
use log::{Level, Metadata, Record};
#[cfg(feature = "message-box")]
use msgbox::IconType;
use std::env;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};

#[cfg(feature = "message-box")]
/// Window title of the message alert box.
const ALERT_DIALOG_TITLE: &str = "Tp-Note";

#[cfg(feature = "message-box")]
lazy_static! {
    /// Window title followed by version.
    pub static ref ALERT_DIALOG_TITLE_LINE: String = format!(
        "{} (v{})",
        &ALERT_DIALOG_TITLE,
        VERSION.unwrap_or("unknown")
    );
}

/// Pops up an error message box and prints `msg`.
/// Blocks until the user closes the window.
#[cfg(feature = "message-box")]
fn popup_alert(msg: &str) {
    let _ = msgbox::create(&ALERT_DIALOG_TITLE_LINE, msg, IconType::Info);
}

lazy_static! {
    /// Some additional debugging information added to the end of error messages.
    pub static ref ERR_MSG_TAIL: String = {
        let mut args_str = String::new();
        for argument in env::args() {
            args_str.push_str(argument.as_str());
            args_str.push(' ');
        };

        format!(
            "\n\
            __________\n\
            Additional technical details:\n\
            *    Command line parameters:\n\
            {}\n\
            *    Configuration file:\n\
            {}",
            args_str,
            CONFIG_PATH
                .as_ref()
                .unwrap_or(&PathBuf::from("no path found"))
                .to_str()
                .unwrap_or_default()
        )
    };
}

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
            AlertService::init(popup_alert);
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
    fn enabled(&self, metadata: &Metadata<'_>) -> bool {
        metadata.level() <= Level::Trace
    }

    fn log(&self, record: &Record<'_>) {
        if self.enabled(record.metadata()) {
            let mut msg = format!("{}:\n{}", record.level(), &record.args().to_string());
            if record.metadata().level() == Level::Error {
                msg.push_str(&ERR_MSG_TAIL);
            };

            // Log this to `stderr`.
            eprintln!("*** {}", msg);

            // Eventually also log as popup alert window.
            #[cfg(feature = "message-box")]
            if !*RUNS_ON_CONSOLE
                && !ARGS.batch
                && ((record.metadata().level() == LevelFilter::Error)
                        // This lock can never get poisoned, so `unwrap()` is safe here.
                        || APP_LOGGER.popup_always_enabled.load(Ordering::SeqCst))
            {
                // We silently ignore failing pushes. We have printed the
                // error message on the console already.
                let _ = AlertService::push_str(msg);
            };
        }
    }

    fn flush(&self) {
        Self::flush();
    }
}
