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
mod filename;
mod filter;
mod note;
#[cfg(feature = "viewer")]
mod sse_server;
#[cfg(feature = "viewer")]
mod viewer;
mod watcher;

extern crate semver;
use crate::config::backup_config_file;
use crate::config::ARGS;
use crate::config::CFG;
use crate::config::CLIPBOARD;
use crate::config::LAUNCH_EDITOR;
use crate::config::LAUNCH_VIEWER;
use crate::config::RUNS_ON_CONSOLE;
use crate::config::STDIN;
use crate::error::AlertDialog;
use crate::filename::MarkupLanguage;
use crate::note::Note;
#[cfg(feature = "viewer")]
use crate::viewer::Viewer;
use anyhow::{anyhow, Context};
use clipboard::ClipboardContext;
use clipboard::ClipboardProvider;
use semver::Version;
use std::env;
use std::fs;
#[cfg(not(target_family = "windows"))]
use std::fs::File;
use std::matches;
use std::path::Path;
use std::path::PathBuf;
use std::process;
use std::process::Command;
use std::process::Stdio;
#[cfg(feature = "viewer")]
use std::thread;

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
const MIN_CONFIG_FILE_VERSION: Option<&'static str> = VERSION;
/// (c) Jens Getreu
const AUTHOR: &str = "(c) Jens Getreu, 2020";
/// Open the note file `path` on disk and reads its YAML front matter.
/// Then calculate from the front matter how the filename should be to
/// be in sync. If it is different, rename the note on disk and return
/// the new filename.
fn synchronize_filename(path: PathBuf) -> Result<PathBuf, anyhow::Error> {
    // parse file again to check for synchronicity with filename
    let n = Note::from_existing_note(&path).context(
        "Failed to parse the note's metadata. \
                  Can not synchronize the note's filename!",
    )?;

    if ARGS.debug {
        eprintln!("*** Debug: Applying template `tmpl_sync_filename`.");
    };
    let new_fqfn = n.render_filename(&CFG.tmpl_sync_filename).context(
        "Failed to render the template `tmpl_sync_filename` in config file. \
                  Can not synchronize the note's filename!",
    )?;

    if !filename::exclude_copy_counter_eq(&path, &new_fqfn) {
        let new_fqfn = filename::find_unused(new_fqfn).context(
            "Can not rename the note's filename to be in sync with its\n\
            YAML header.",
        )?;
        // rename file
        fs::rename(&path, &new_fqfn)?;
        if ARGS.debug {
            eprintln!("*** Debug: File renamed to {:?}", new_fqfn);
        };
        Ok(new_fqfn)
    } else {
        Ok(path)
    }
}

#[inline]
/// Create a new note by inserting `Tp-Note`'s environment in a template.
/// If the note to be created exists already, append a so called `copy_counter`
/// to the filename and try to save it again. In case this does not succeed either,
/// increment the `copy_counter` until a free filename is found.
fn create_new_note_or_synchronize_filename(path: PathBuf) -> Result<PathBuf, anyhow::Error> {
    // First generate a new note (if it does not exist), then parse its front_matter
    // and finally rename the file, if it is not in sync with its front matter.
    if path.is_dir() {
        let (n, new_fqfn) = if STDIN.is_empty() && CLIPBOARD.is_empty() {
            // CREATE A NEW NOTE WITH `TMPL_NEW_CONTENT` TEMPLATE
            let n = Note::from_content_template(&path, &CFG.tmpl_new_content)
                .context("Can not render the template `tmpl_new_content` in config file.")?;
            let new_fqfn = n
                .render_filename(&CFG.tmpl_new_filename)
                .context("Can not render the template `tmpl_new_filename` in config file.")?;
            if ARGS.debug {
                eprintln!(
                    "*** Debug: Applying templates `tmpl_new_content` and `tmpl_new_filename`."
                );
            }
            (n, new_fqfn)
        } else if !STDIN.header.is_empty() || !CLIPBOARD.header.is_empty() {
            // CREATE A NEW NOTE BASED ON CLIPBOARD OR INPUT STREAM
            // (only if there is a valid YAML front matter)
            let n = Note::from_content_template(&path, &CFG.tmpl_copy_content)
                // CREATE A NEW NOTE WITH `TMPL_COPY_CONTENT` TEMPLATE
                .context("Can not render the template `tmpl_copy_content` in config file.")?;
            let new_fqfn = n
                .render_filename(&CFG.tmpl_copy_filename)
                .context("Can not render the template `tmpl_copy_filename` in config file.")?;
            if ARGS.debug {
                eprintln!(
                    "*** Debug: Applying templates: `tmpl_copy_content`, `tmpl_copy_filename`"
                );
            };
            (n, new_fqfn)
        } else {
            // CREATE A NEW NOTE BASED ON CLIPBOARD OR INPUT STREAM
            let n = Note::from_content_template(&path, &CFG.tmpl_clipboard_content)
                // CREATE A NEW NOTE WITH `TMPL_CLIPBOARD_CONTENT` TEMPLATE
                .context("Can not render the template `tmpl_clipboard_content` in config file.")?;
            let new_fqfn = n
                .render_filename(&CFG.tmpl_clipboard_filename)
                .context("Can not render the template `tmpl_clipboard_filename` in config file.")?;
            if ARGS.debug {
                eprintln!(
                    "*** Debug: Applying templates: `tmpl_clipboard_content`, `tmpl_clipboard_filename`"
                );
            };
            (n, new_fqfn)
        };

        // Check if the filename is not taken already
        let new_fqfn = filename::find_unused(new_fqfn)?;

        // Write new note on disk.
        n.content.write_to_disk(new_fqfn)
    } else {
        let extension_is_known = !matches!(MarkupLanguage::from(None, &path), MarkupLanguage::None);

        if extension_is_known {
            // SYNCHRONIZE FILENAME
            // `path` points to an existing tp-note file.
            // Check if in sync with its filename:
            Ok(synchronize_filename(path)?)
        } else {
            // ANNOTATE FILE: CREATE NEW NOTE WITH TMPL_ANNOTATE_CONTENT TEMPLATE
            // `path` points to a foreign file type that will be annotated.
            if ARGS.debug {
                eprintln!(
                    "*** Debug: Applying templates `tmpl_annotate_content` and `tmpl_annotate_filename`."
                );
            };
            let n = Note::from_content_template(&path, &CFG.tmpl_annotate_content)
                .context("Can not render the template `tmpl_annotate_content` in config file.")?;
            let new_fqfn = n
                .render_filename(&CFG.tmpl_annotate_filename)
                .context("Can not render the template `tmpl_annotate_filename` in config file.")?;

            // Check if the filename is not taken already
            let new_fqfn = filename::find_unused(new_fqfn)?;

            // Write new note on disk.
            n.content.write_to_disk(new_fqfn)
        }
    }
}

#[inline]
/// Launch some external editor. The editor can be chosen through
/// `tp-note`'s configuration file.
fn launch_editor(path: &Path) -> Result<(), anyhow::Error> {
    // Both lists have always the same number of items.
    let mut args_list = Vec::new();
    let mut executable_list = Vec::new();

    // Choose the right parameter list.
    let editor_args = match *RUNS_ON_CONSOLE {
        true => &CFG.editor_console_args,
        false => &CFG.editor_args,
    };

    // Prepare launch of editor/viewer.

    for app in &*editor_args {
        executable_list.push(&app[0]);
        let mut args: Vec<&str> = Vec::new();
        for s in app[1..].iter() {
            args.push(s);
        }
        args.push(
            path.to_str()
                .ok_or_else(|| anyhow!(format!("Failed to convert the argument: {:?}", path)))?,
        );
        args_list.push(args);
    }

    // Launch editor/viewer.
    if ARGS.debug {
        eprintln!("*** Debug: Opening file {:?}", path);
    };

    let mut executable_found = false;
    for i in 0..executable_list.len() {
        if ARGS.debug {
            eprint!(
                "*** Debug: Trying to launch the executable: {}",
                executable_list[i]
            );
            for j in &args_list[i] {
                eprint!(" \"{}\"", j);
            }
            eprintln!()
        };

        // Check if this is a `flatpak run <app>` command.
        if executable_list[i].starts_with("flatpak")
            && args_list[i].len() >= 3
            && args_list[i][0] == "run"
        {
            // Check if the flatpak is installed on this system with `flatpak info <app>`.
            if let Ok(ecode) = Command::new(executable_list[i])
                .args(&["info", args_list[i][1]])
                .stderr(Stdio::null())
                .stdout(Stdio::null())
                .status()
            {
                if !ecode.success() {
                    // This is a flatpak command, but the application is not installed on this system.
                    // Silently ignore this flatpak command.
                    if ARGS.debug {
                        eprintln!(
                            "*** Debug: Flatpak executable \"{}\" not found.",
                            args_list[i][1]
                        );
                    }
                    continue;
                };
            };
        };

        // Connect `stdin` of child process to `/dev/tty`.
        #[cfg(not(target_family = "windows"))]
        let (config_stdin, config_stdout) = if *RUNS_ON_CONSOLE {
            if let Ok(file) = File::open("/dev/tty") {
                (Stdio::from(file), Stdio::inherit())
            } else {
                (Stdio::null(), Stdio::null())
            }
        } else {
            (Stdio::null(), Stdio::null())
        };
        #[cfg(target_family = "windows")]
        let (config_stdin, config_stdout) = (Stdio::null(), Stdio::null());

        let child = Command::new(&executable_list[i])
            .args(&args_list[i])
            .stdin(config_stdin)
            .stdout(config_stdout)
            .stderr(Stdio::null())
            .spawn();

        if let Ok(mut child) = child {
            let ecode = child.wait().context("Failed to wait on editor to close.")?;

            if !ecode.success() {
                return Err(anyhow!(format!(
                    "The external file editor did not terminate gracefully:\n\
                     \t{}\n\
                     \n\
                     Edit the variable `{}` in Tp-Note's configuration file\n\
                     and correct the following:\n\
                     \t{:?}",
                    ecode.to_string(),
                    if *RUNS_ON_CONSOLE {
                        "editor_console_args"
                    } else {
                        "editor_args"
                    },
                    &*editor_args[i],
                )));
            };

            executable_found = true;
            break;
        } else if ARGS.debug {
            eprintln!(
                "*** Debug: Executable \"{}\" not found.",
                executable_list[i]
            );
        }
    }

    if !executable_found {
        return Err(anyhow!(format!(
            "None of the following external file editor\n\
             applications can be found on your system:\n\
             \t{:?}\n\
             \n\
             Register some already installed file editor in the variable\n\
             `{}` in Tp-Note's configuration file  or \n\
             install one of the above listed applications.",
            &executable_list,
            // Choose the right parameter list.
            match *RUNS_ON_CONSOLE {
                true => "editor_console_args",
                false => "editor_args",
            }
        )));
    };

    Ok(())
}

#[inline]
/// Launches a file watcher and Markdown renderer and displays the
/// result in the system's default browser.
#[cfg(feature = "viewer")]
fn launch_viewer(path: &Path) -> Result<(), anyhow::Error> {
    let p = path.to_path_buf();
    if *LAUNCH_EDITOR {
        thread::spawn(move || Viewer::run(p));
    } else {
        Viewer::run(p);
    }
    Ok(())
}

/// Run Tp-Note and return the (modified) path to the (new) note file.
/// 1. Create a new note by inserting `tp-note`'s environment in a template.
/// 2. If the note to be created exists already, open it, read the YAML front
///    matter and synchronize the filename if necessary.
/// 3. Open the new note in an external editor (configurable).
/// 4. Read the front matter again and resynchronize the filename if necessary.
#[inline]
fn run() -> Result<PathBuf, anyhow::Error> {
    // process arg = `--version`
    if ARGS.version {
        eprintln!("Version {}, {}", VERSION.unwrap_or("unknown"), AUTHOR);
        process::exit(0);
    };

    // process arg = <path>
    let path = if let Some(p) = &ARGS.path {
        p.canonicalize().with_context(|| {
            format!(
                "invalid <path>: `{}`",
                &ARGS
                    .path
                    .as_ref()
                    .unwrap_or(&PathBuf::from("unknown"))
                    .display()
            )
        })?
    } else {
        env::current_dir()?
    };

    let path = create_new_note_or_synchronize_filename(path)?;

    #[cfg(feature = "viewer")]
    if *LAUNCH_VIEWER {
        launch_viewer(&path)?;
    }

    if *LAUNCH_EDITOR {
        launch_editor(&path)?;

        let path = synchronize_filename(path)?;

        // Delete clipboard
        if CFG.clipboard_read_enabled && CFG.clipboard_empty_enabled && !*RUNS_ON_CONSOLE {
            let ctx: Option<ClipboardContext> = ClipboardProvider::new().ok();
            if let Some(mut ctx) = ctx {
                ctx.set_contents("".to_owned()).unwrap_or_default();
            };
        }
        Ok(path)
    } else {
        Ok(path)
    }
}

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
                // Unwrap path argument.
                let no_path = PathBuf::new();
                let path: &Path = ARGS.path.as_ref().unwrap_or(&no_path);

                if path.is_file() {
                    AlertDialog::print_error(&format!(
                        "ERROR:\n\
                    ---\n\
                    {:?}\n\
                    \n\
                    Please correct the error.
                    Trying to start the file editor without synchronization...",
                        e
                    ));
                    let _ = launch_editor(path);
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
            }
            process::exit(1);
        }
        Ok(path) => {
            println!("{}", path.to_str().unwrap_or_default());
        }
    };
}
