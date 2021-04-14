//! Prints error messages and exceptional states.

#[cfg(feature = "message-box")]
extern crate msgbox;

#[cfg(feature = "message-box")]
use crate::config::ARGS;
use crate::config::CONFIG_PATH;
#[cfg(feature = "message-box")]
use crate::config::RUNS_ON_CONSOLE;
use crate::VERSION;
use lazy_static::lazy_static;
use log::LevelFilter;
use log::{Level, Metadata, Record};
#[cfg(feature = "message-box")]
use msgbox::IconType;
use std::env;
use std::path::PathBuf;

/// Console logger.
pub struct AppLogger;
pub static APP_LOGGER: AppLogger = AppLogger;

/// Initialize logger.
impl AppLogger {
    pub fn init() {
        log::set_logger(&APP_LOGGER).unwrap();
        if let Some(level) = ARGS.debug {
            log::set_max_level(level);
        } else {
            log::set_max_level(LevelFilter::Error);
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
            if record.metadata().level() == Level::Error {
                let msg = format!(
                    "{}:\n{}",
                    record.level(),
                    &AlertDialog::format_error(&record.args().to_string())
                );
                eprintln!("*** {}", msg);
                // In addition, we open an alert window.
                AlertDialog::print_error(&msg);
            } else {
                eprintln!("*** {}: {}", record.level(), record.args());
            }
        }
    }
    fn flush(&self) {}
}

/// Window title for error box.
const ALERT_DIALOG_TITLE: &str = "Tp-Note";

lazy_static! {
    /// Window title followed by version.
    pub static ref ALERT_DIALOG_TITLE_LINE: String = format!(
        "{} (v{})",
        &ALERT_DIALOG_TITLE,
        VERSION.unwrap_or("unknown")
    );
}

/// Empty struct. This crate is stateless.
pub struct AlertDialog {}

impl AlertDialog {
    /// Pops up an error message box and prints `msg`.
    fn print_error(msg: &str) {
        // Print the same message also to console in case
        // the window does not pop up due to missing
        // libraries.
        //Self::print_error_console(msg);
        // Popup window.
        #[cfg(feature = "message-box")]
        if !*RUNS_ON_CONSOLE && !ARGS.batch {
            let _ = msgbox::create(&*ALERT_DIALOG_TITLE_LINE, &msg, IconType::Info);
        }
    }

    /// Add a footer with additional debugging information, such as
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
