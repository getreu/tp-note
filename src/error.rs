//! Prints error messages and exceptional states.

extern crate msgbox;

use crate::ALERT_DIALOG_TITLE;
use crate::VERSION;
use msgbox::IconType;

/// Empty struct. This crate is stateless.
pub struct AlertDialog {}

impl AlertDialog {
    /// Pops up a message box and prints `msg`.
    pub fn print_message(msg: &str) {
        let title = format!("{} (v{})", ALERT_DIALOG_TITLE, VERSION.unwrap_or("unknown"));
        // Print the same message also to console in case
        // the window does not pop up due to missing
        // libraries.
        Self::print_message_console(msg);
        // Popup window.
        msgbox::create(&title, msg, IconType::Info);
    }

    /// Prints `msg` on console.
    pub fn print_message_console(msg: &str) {
        let title = format!("{} (v{})", ALERT_DIALOG_TITLE, VERSION.unwrap_or("unknown"));
        // Print the same message also to console in case
        // the window does not pop up due to missing
        // libraries.
        eprintln!("{}\n\n{}", title, msg);
    }
}
