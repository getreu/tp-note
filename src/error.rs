//! Prints error messages and exceptional states.

extern crate msgbox;

use crate::config::CONFIG_PATH;
use crate::config::RUNS_ON_CONSOLE;
use crate::VERSION;
use lazy_static::lazy_static;
use msgbox::IconType;
use std::env;

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
    pub fn print_error(msg: &str) {
        // Print the same message also to console in case
        // the window does not pop up due to missing
        // libraries.
        Self::print_error_console(msg);
        // Popup window.
        if !*RUNS_ON_CONSOLE {
            msgbox::create(
                &*ALERT_DIALOG_TITLE_LINE,
                &Self::format_error(msg),
                IconType::Info,
            );
        }
    }

    /// Prints an error `msg` on console.
    pub fn print_error_console(msg: &str) {
        // Print the same message also to console in case
        // the window does not pop up due to missing
        // libraries.
        eprintln!("{}\n{}", *ALERT_DIALOG_TITLE_LINE, &Self::format_error(msg));
    }

    /// Pops up a message box and prints `msg`.
    pub fn print(msg: &str) {
        // Print the same message also to console in case
        // the window does not pop up due to missing
        // libraries.
        Self::print_console(msg);
        // Popup window.
        if !*RUNS_ON_CONSOLE {
            msgbox::create(&*ALERT_DIALOG_TITLE_LINE, msg, IconType::Info);
        }
    }

    /// Prints `msg` on console.
    pub fn print_console(msg: &str) {
        // Print the same message also to console in case
        // the window does not pop up due to missing
        // libraries.
        eprintln!("{}\n{}", &*ALERT_DIALOG_TITLE_LINE, msg);
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
            *    Configuration file path:\n\
            {}",
            args_str,
            &*CONFIG_PATH.to_str().unwrap_or_default()
        ));
        s
    }
}
