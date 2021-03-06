#![windows_subsystem = "windows"]
//! _Tp-Note_ is a note taking tool and a template system, that consistently
//! synchronizes the note's metadata with its filename. _Tp-Note_ collects
//! various information about its environment and the clipboard and stores them
//! in variables. New notes are created by filling these variables in predefined
//! and customizable `Tera`-templates. In case `<path>` points to an existing
//! _Tp-Note_-file, the note's metadata is analysed and, if necessary, its
//! filename is modified. For all other file types, _Tp-Note_ creates a new note
//! that annotates the file `<path>` points to. If `<path>` is a directory (or,
//! when omitted the current working directory), a new note is created in that
//! directory. After creation, _Tp-Note_ launches an external editor of your
//! choice. Although the note's structure follows _Pandoc_'s conventions, it is not
//! tied to any specific Markup language.

#[cfg(feature = "message-box")]
mod alert_service;
mod config;
mod content;
mod error;
mod file_editor;
mod filename;
mod filter;
mod logger;
mod note;
mod process_ext;
mod settings;
#[cfg(feature = "viewer")]
mod viewer;
mod workflow;

extern crate semver;
use crate::config::backup_config_file;
use crate::config::CFG;
use crate::config::CFG_FILE_LOADING;
use crate::logger::AppLogger;
use crate::settings::ARGS;
use crate::workflow::run;
use error::FileError;
use semver::Version;
use std::process;

/// Use the version number defined in `../Cargo.toml`.
const VERSION: Option<&'static str> = option_env!("CARGO_PKG_VERSION");
/// Set the minimum required config file version that is compatible with this Tp-Note version.
///
/// Examples how to use this constant. Choose one of the following:
/// 1. Require some minimum version of the config file.
///    Abort if not satisfied.
///
///    ```no_run
///    const MIN_CONFIG_FILE_VERSION: Option<&'static str> = Some("1.5.1");
///    ```
///
/// 2. Require the config file to be of the same version as this binary. Abort if not satisfied.
///
///    ```no_run
///    const MIN_CONFIG_FILE_VERSION: Option<&'static str> = VERSION;
///    ```
///
/// 3. Disable minimum version check; all config file versions are allowed.
///
///    ```no_run
///    const MIN_CONFIG_FILE_VERSION: Option<&'static str> = None;
///    ```
///
const MIN_CONFIG_FILE_VERSION: Option<&'static str> = VERSION;
/// (c) Jens Getreu
const AUTHOR: &str = "(c) Jens Getreu, 2020-2021";

/// Print some error message if `run()` does not complete.
/// Exit prematurely if the configuration file version does
/// not match the program version.
fn main() {
    // Setup logger.
    AppLogger::init();

    // Read configuration file, or write one if none exists.
    lazy_static::initialize(&CFG);

    // Set the debug level. Only use config file value if
    // no command-line-option `--debug` is present.
    let level = ARGS.debug.unwrap_or(CFG.arg_default.debug);
    AppLogger::set_max_level(level);

    // This eventually will extend the error reporting with more
    // popup alert windows.
    AppLogger::set_popup_always_enabled(ARGS.popup || CFG.arg_default.popup);

    // Check if the config file loading was successful.
    let cfg_file_loading = &*CFG_FILE_LOADING.read().unwrap();
    let cfg_file_loading_err = cfg_file_loading.as_ref().err().map(|e| e.to_string());

    // Check if we can parse the version number in there.
    let cfg_file_version = Version::parse(&*CFG.version);
    let cfg_file_version_err = cfg_file_version.as_ref().err().map(|e| e.to_string());

    // This is `Some::String` if one of them is `Err`.
    let cfg_err = cfg_file_loading_err.or(cfg_file_version_err);

    let config_file_version = match cfg_err {
        // This is always `Some::Version` because none of them are `Err`.
        None => cfg_file_version.ok(),

        // One of them is `Err`, we do not care who.
        Some(e) => {
            log::error!("{}", FileError::ConfigFileLoadParseWrite { error: e });

            // Move erroneous config file away.
            if let Err(e) = backup_config_file() {
                log::error!(
                    "{}",
                    FileError::ConfigFileBackup {
                        error: e.to_string()
                    }
                    .to_string()
                );
                AppLogger::flush();
                process::exit(5);
            };

            // As we have an error, we indicate that there is no version.
            None
        }
    };

    // Is version number in the configuration file high enough?
    if let Some(config_file_version) = config_file_version {
        if config_file_version < Version::parse(MIN_CONFIG_FILE_VERSION.unwrap_or("0.0.0")).unwrap()
        {
            log::error!(
                "{}",
                FileError::ConfigFileVersionMismatch {
                    config_file_version: config_file_version.to_string(),
                    min_version: MIN_CONFIG_FILE_VERSION.unwrap_or("0.0.0").to_string(),
                }
                .to_string()
            );
            if let Err(e) = backup_config_file() {
                log::error!(
                    "{}",
                    FileError::ConfigFileBackup {
                        error: e.to_string()
                    }
                    .to_string()
                );
                AppLogger::flush();
                process::exit(5);
            };
        };
    };

    // Run Tp-Note.
    match run() {
        Err(e) => {
            // Something went wrong.
            log::error!("{}", e);
            AppLogger::flush();
            process::exit(1);
        }

        // Print `path` unless `--export=-`.
        Ok(path) => {
            if let Some(p) = &ARGS.export {
                if p.as_os_str().to_str().unwrap_or_default() != "-" {
                    println!("{}", path.to_str().unwrap_or_default());
                }
            } else {
                println!("{}", path.to_str().unwrap_or_default());
            }
        }
    };

    // Wait if there are still error messages windows open.
    AppLogger::flush();
}
