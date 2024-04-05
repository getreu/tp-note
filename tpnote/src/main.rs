#![windows_subsystem = "windows"]
#![allow(clippy::vec_init_then_push)]
//! _Tp-Note_ is a note taking tool and a template system, that consistently
//! synchronizes the note's metadata with its filename.
//! _Tp-Note_'s main design goal is to convert some input text -
//! usually provided by the system's clipboard - into a Markdown note file, with
//! a descriptive YAML header and meaningful filename.
//! _Tp-Note_ collects
//! various information about its environment and the clipboard and stores them
//! in variables. New notes are created by filling these variables in predefined
//! and customizable `Tera`-templates. In case `<path>` points to an existing
//! _Tp-Note_-file, the note's metadata is analyzed and, if necessary, its
//! filename is modified. For all other file types, _Tp-Note_ creates a new note
//! that annotates the file `<path>` points to. If `<path>` is a directory (or,
//! when omitted the current working directory), a new note is created in that
//! directory. After creation, _Tp-Note_ launches an external editor of your
//! choice. Although the note's structure follows _Pandoc_'s conventions, it is
//! not tied to any specific Markup language.

#[cfg(feature = "message-box")]
mod alert_service;
mod clipboard;
mod config;
mod error;
mod file_editor;
mod logger;
mod process_ext;
mod settings;
mod template;
#[cfg(feature = "viewer")]
mod viewer;
mod workflow;

#[cfg(feature = "message-box")]
use crate::alert_service::AlertService;
use crate::clipboard::TpClipboard;
use crate::config::Cfg;
use crate::config::AUTHOR;
use crate::config::CFG;
use crate::config::CFG_FILE_LOADING;
use crate::config::CONFIG_PATHS;
use crate::config::COPYRIGHT_FROM;
use crate::config::PKG_VERSION;
use crate::error::WorkflowError;
use crate::logger::AppLogger;
use crate::settings::ARGS;
use crate::settings::LAUNCH_EDITOR;
#[cfg(feature = "message-box")]
use crate::settings::RUNS_ON_CONSOLE;
use crate::workflow::run;
use config::MIN_CONFIG_FILE_VERSION;
use error::ConfigFileError;
use semver::Version;
use serde::Serialize;
use settings::CLIPBOARD;
use std::path::Path;
use std::process;
use tpnote_lib::error::NoteError;

#[derive(Debug, PartialEq, Serialize)]
struct About {
    version: String,
    features: Vec<String>,
    searched_config_file_paths: Vec<String>,
    sourced_config_files: Vec<String>,
    copyright: String,
}

/// Print some error message if `run()` does not complete.
/// Exit prematurely if the configuration file version does
/// not match the program version.
fn main() {
    // Read the clipboard before starting the logger.
    lazy_static::initialize(&CLIPBOARD);

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
    let cfg_file_loading = &*CFG_FILE_LOADING.read();
    let cfg_file_loading_err = cfg_file_loading.as_ref().err().map(|e| e.to_string());

    // Check if we can parse the version number in there.
    let cfg_file_version = Version::parse(&CFG.version);
    let cfg_file_version_err = cfg_file_version.as_ref().err().map(|e| e.to_string());

    // This is `Some::String` if one of them is `Err`.
    let cfg_err = cfg_file_loading_err.or(cfg_file_version_err);

    let config_file_version = match cfg_err {
        // This is always `Some::Version` because none of them are `Err`.
        None => cfg_file_version.ok(),

        // One of them is `Err`, we do not care who.
        Some(e) => {
            log::error!("{}", ConfigFileError::ConfigFileLoadParse { error: e });

            // Move erroneous config file away.
            if let Err(e) = Cfg::backup_and_remove_last() {
                log::error!(
                    "{}",
                    ConfigFileError::ConfigFileBackup {
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
                ConfigFileError::ConfigFileVersionMismatch {
                    config_file_version: config_file_version.to_string(),
                    min_version: MIN_CONFIG_FILE_VERSION.unwrap_or("0.0.0").to_string(),
                }
                .to_string()
            );
            if let Err(e) = Cfg::backup_and_remove_last() {
                log::error!(
                    "{}",
                    ConfigFileError::ConfigFileBackup {
                        error: e.to_string()
                    }
                    .to_string()
                );
                AppLogger::flush();
                process::exit(6);
            };
        };
    };

    // Process `arg = `--default-config`.
    if let Some(path) = &ARGS.config_defaults {
        let path = Path::new(&path);

        if let Err(e) = Cfg::write_default_to_file_or_stdout(path) {
            log::error!(
                "{}",
                ConfigFileError::ConfigFileWrite {
                    error: e.to_string()
                }
                .to_string()
            );
            AppLogger::flush();
            process::exit(5);
        };
        // Exit.
        AppLogger::flush();
        process::exit(0);
    }

    // Process `arg = `--version`.
    // The output is YAML formatted for further automatic processing.
    if ARGS.version {
        #[allow(unused_mut)]
        let mut features = Vec::new();
        #[cfg(feature = "lang-detection")]
        features.push("lang-detection".to_string());
        #[cfg(feature = "message-box")]
        features.push("message-box".to_string());
        #[cfg(feature = "read-clipboard")]
        features.push("read-clipboard".to_string());
        #[cfg(feature = "renderer")]
        features.push("renderer".to_string());
        #[cfg(feature = "viewer")]
        features.push("viewer".to_string());

        let about = About {
            version: PKG_VERSION.unwrap_or("unknown").to_string(),
            features,
            searched_config_file_paths: CONFIG_PATHS
                .iter()
                .map(|p| p.to_str().unwrap_or_default().to_owned())
                .collect(),
            sourced_config_files: CONFIG_PATHS
                .iter()
                .filter(|p| p.exists())
                .map(|p| p.to_str().unwrap_or_default().to_owned())
                .collect(),
            copyright: format!(
                "© {}-{} {}",
                COPYRIGHT_FROM,
                time::OffsetDateTime::now_utc().year(),
                AUTHOR.unwrap()
            ),
        };

        let mut msg = serde_yaml::to_string(&about).unwrap_or_else(|_| "unknown".to_string());
        msg.push_str("---");

        // Print on console.
        println!("{}", msg);

        // Print in alert box.
        #[cfg(feature = "message-box")]
        if !*RUNS_ON_CONSOLE && !ARGS.batch {
            let _ = AlertService::push_str(msg);
        };
        AppLogger::flush();
        process::exit(0);
    };

    //
    // Run Tp-Note.
    let res = run();
    match res {
        Err(ref e) => {
            // Something went wrong. Inform user.
            log::error!("{}", e);
        }

        // Print `path` unless `--export=-`.
        Ok(ref path) => {
            if let Some(p) = &ARGS.export {
                if p.display().to_string() != "-" {
                    println!("{}", path.display());
                }
            } else {
                println!("{}", path.display());
            }
        }
    };

    // Wait if there are still error messages windows open.
    AppLogger::flush();

    // Delete clipboard content.
    if (*LAUNCH_EDITOR && !ARGS.batch && CFG.clipboard.read_enabled && CFG.clipboard.empty_enabled)
        || matches!(
            &res,
            Err(WorkflowError::Note(NoteError::InvalidInputYaml { .. }))
        )
    {
        TpClipboard::empty();
    }

    if res.is_err() {
        process::exit(1);
    }
}
