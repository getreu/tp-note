//! Prints error messages and exceptional states.

#[cfg(feature = "message-box")]
use crate::alert_service::AlertService;
use crate::config::CARGO_BIN_NAME;
#[cfg(feature = "message-box")]
use crate::settings::ARGS;
#[cfg(feature = "message-box")]
use crate::settings::RUNS_ON_CONSOLE;
use crate::CONFIG_PATHS;
#[cfg(feature = "message-box")]
use crate::PKG_VERSION;
use log::LevelFilter;
use log::{Level, Metadata, Record};
#[cfg(all(unix, not(target_os = "macos")))]
#[cfg(feature = "message-box")]
use notify_rust::Hint;
#[cfg(not(target_os = "windows"))]
#[cfg(feature = "message-box")]
use notify_rust::{Notification, Timeout};
use parking_lot::RwLock;
use std::env;
use std::sync::LazyLock;
#[cfg(target_os = "windows")]
#[cfg(feature = "message-box")]
use win_msgbox::{information, Okay};

#[cfg(feature = "message-box")]
/// Window title of the message alert box.
const ALERT_DIALOG_TITLE: &str = "Tp-Note";

#[cfg(feature = "message-box")]
/// Window title followed by version.
pub static ALERT_DIALOG_TITLE_LINE: LazyLock<String> = LazyLock::new(|| {
    format!(
        "{} (v{})",
        &ALERT_DIALOG_TITLE,
        PKG_VERSION.unwrap_or("unknown")
    )
});

/// Pops up an error message notification and prints `msg`.
/// Blocks until the user closes the window.
/// Under Linux no notifications will be shown when
/// `log::max_level=Level::Trace`.
#[cfg(not(target_os = "windows"))]
#[cfg(feature = "message-box")]
fn popup_alert(msg: &str) {
    if log::max_level() == Level::Trace {
        return;
    }

    let mut n = Notification::new();
    let n = n
        .summary(&ALERT_DIALOG_TITLE_LINE)
        .body(msg)
        .icon("tpnote")
        .appname("tpnote");

    #[cfg(all(unix, not(target_os = "macos")))]
    let n = n.hint(Hint::Resident(true));

    if let Ok(_handle) = n
        // Does not work on Kde.
        .timeout(Timeout::Never) // Works on Kde and Gnome.
        .show()
    {
        // // Only available in Linux.
        // _handle.wait_for_action(|_action| {
        //     if "__closed" == _action {
        //         println!("the notification was closed")
        //     }
        // })
    };
}
/// Pops up an error message box and prints `msg`.
/// Blocks until the user closes the window.
#[cfg(target_os = "windows")]
#[cfg(feature = "message-box")]
fn popup_alert(msg: &str) {
    // Silently ignore `show()` error.
    let _ = information::<Okay>(msg)
        .title(&ALERT_DIALOG_TITLE_LINE)
        .show();
}

/// Some additional debugging information added to the end of error messages.
pub static ERR_MSG_TAIL: LazyLock<String> = LazyLock::new(|| {
    use std::fmt::Write;

    let mut args_str = String::new();
    for argument in env::args() {
        args_str.push_str(argument.as_str());
        args_str.push(' ');
    }

    format!(
        "\n\
            \n\
            Additional technical details:\n\
            *    Command line parameters:\n\
            {}\n\
            *    Sourced configuration files:\n\
            {}",
        args_str,
        CONFIG_PATHS
            .iter()
            .filter(|p| p.exists())
            .map(|p| p.to_str().unwrap_or_default())
            .fold(String::new(), |mut output, p| {
                let _ = writeln!(output, "{p}");
                output
            })
    )
});

/// If `true`, all future log events will trigger the opening of a popup
/// alert window. Otherwise only `Level::Error` will do.
static APP_LOGGER_ENABLE_POPUP: RwLock<bool> = RwLock::new(false);

pub struct AppLogger;
static APP_LOGGER: AppLogger = AppLogger;

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
        // Release lock immediately.
        *APP_LOGGER_ENABLE_POPUP.write() = popup;
    }

    /// Blocks until the `AlertService` is not busy any more. This should be
    /// executed before quitting the application because there might be still
    /// queued error messages the uses has not seen yet.
    /// Once flushed, no more logs are recorded.
    pub fn flush() {
        #[cfg(feature = "message-box")]
        if !*RUNS_ON_CONSOLE && !ARGS.batch {
            // If ever there is still a message window open, this will block.
            AlertService::flush();
            log::set_max_level(LevelFilter::Off);
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

            // Only log Tp-Note errors. Silently ignore others.
            if !record.metadata().target().starts_with(CARGO_BIN_NAME) {
                return;
            }

            // Log this to `stderr`.
            eprintln!("*** {}", msg);

            // Eventually also log as popup alert window.
            #[cfg(feature = "message-box")]
            if !*RUNS_ON_CONSOLE
                && !ARGS.batch
                && ((record.metadata().level() == LevelFilter::Error)
                        // Release lock immediately.
                        || *APP_LOGGER_ENABLE_POPUP.read())
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
