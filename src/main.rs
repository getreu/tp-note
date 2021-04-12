#![windows_subsystem = "windows"]
//! _Tp-Note_ is a note-taking-tool and a template system, that consistently
//! synchronizes the note's meta-data with its filename. `tp-note` collects
//! various information about its environment and the clipboard and stores them
//! in variables. New notes are created by filling these variables in predefined
//! and customizable `Tera`-templates. In case `<path>` points to an existing
//! `tp-note`-file, the note's meta-data is analysed and, if necessary, its
//! filename is modified. For all other file types, `tp-note` creates a new note
//! that annotates the file `<path>` points to. If `<path>` is a directory (or,
//! when omitted the current working directory), a new note is created in that
//! directory. After creation, `tp-note` launches an external editor of your
//! choice. Although the note's structure follows `pandoc`-conventions, it is not
//! tied to any specific Markup language.

mod config;
mod content;
mod error;
mod file_editor;
mod filename;
mod filter;
mod note;
mod process_ext;
#[cfg(feature = "viewer")]
mod viewer;
mod workflow;

extern crate semver;
use crate::config::backup_config_file;
use crate::config::ARGS;
use crate::config::CFG;
use crate::error::AlertDialog;
use crate::workflow::run;
use semver::Version;
use std::process;

/// Use the version-number defined in `../Cargo.toml`.
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
const MIN_CONFIG_FILE_VERSION: Option<&'static str> = Some("1.11.0");
/// (c) Jens Getreu
const AUTHOR: &str = "(c) Jens Getreu, 2020-2021";

/// Print some error message if `run()` does not complete.
/// Exit prematurely if the configuration file version does
/// not match the program version.
fn main() {
    // If we could not load or parse the config file, then
    // `CFG.version` does not contain a version number, but an error message.
    let config_file_version = Version::parse(&CFG.version).unwrap_or_else(|_| {
        AlertDialog::print_error(
            format!(
                "NOTE: unable to load, parse or write the configuration file\n\
                ---\n\
                Reason:\n\
                \t{}\n\n\
                Note: this error may occur after upgrading Tp-Note due\n\
                to some incompatible configuration file changes.\n\
                \n\
                For now, Tp-Note backs up the existing configuration\n\
                file and next time it starts, it will create a new one\n\
                with default values.",
                CFG.version
            )
            .as_str(),
        );
        if let Err(e) = backup_config_file() {
            AlertDialog::print_error(&format!(
                "ERROR: unable to backup and delete the erroneous configuration file\n\
                ---\n\
                \t{}\n\
                \n\
                Please do it manually.",
                e
            ));
            process::exit(5);
        };

        // As we just created the config file, config_file_version is VERSION.
        Version::parse(VERSION.unwrap_or("0.0.0")).unwrap_or(Version::new(0, 0, 0))
    });

    // Is version number in the configuration file high enough?
    if config_file_version < Version::parse(MIN_CONFIG_FILE_VERSION.unwrap_or("0.0.0")).unwrap() {
        AlertDialog::print_error(&format!(
            "NOTE: configuration file version mismatch:\n---\n\
                Configuration file version: \'{}\'\n\
                Minimum required configuration file version: \'{}\'\n\
                \n\
                For now, Tp-Note backs up the existing configuration\n\
                file and next time it starts, it will create a new one\n\
                with default values.",
            CFG.version,
            MIN_CONFIG_FILE_VERSION.unwrap_or("0.0.0"),
        ));
        if let Err(e) = backup_config_file() {
            AlertDialog::print_error(&format!(
                "ERROR: unable to backup and delete the erroneous configuration file\n\
                ---\n\
                \t{}\n\
                \n\
                Please do it manually.",
                e
            ));
            process::exit(5);
        };
    };

    // Run Tp-Note.
    match run() {
        Err(e) => {
            // Something went wrong.

            if ARGS.batch {
                AlertDialog::print_error_console(&format!(
                    "ERROR:\n\
                ---\n\
                {:?}",
                    e
                ));
            } else {
                AlertDialog::print_error(&format!(
                    "ERROR:\n\
                    ---\n\
                    {:?}\n\
                    \n\
                    Please correct the error and start again.",
                    e
                ));
            }
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
}
