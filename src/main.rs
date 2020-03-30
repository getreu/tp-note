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
//! tied to any specific markup language.

mod config;
mod content;
mod filter;
mod note;

extern crate msgbox;
use crate::config::print_message;
use crate::config::print_message_console;
use crate::config::ARGS;
use crate::config::CFG;
use crate::config::CLIPBOARD;
use crate::note::Note;
use anyhow::{anyhow, Context};
use clipboard::ClipboardContext;
use clipboard::ClipboardProvider;
use std::env;
use std::fs;
use std::path::Path;
use std::path::PathBuf;
use std::process;
use std::process::Command;

/// Use the version-number defined in `../Cargo.toml`.
const VERSION: Option<&'static str> = option_env!("CARGO_PKG_VERSION");
/// (c) Jens Getreu
const AUTHOR: &str = "(c) Jens Getreu, 2020";
/// Window title for error box.
const MESSAGE_ALERT_WINDOW_TITLE: &str = "Tp-Note Application Error";

/// Opens the note file `path` on disk and reads its YAML front matter.
/// Then calculate from the front matter how the filename should be to
/// be in sync. If it is different, rename the note on disk and return
/// the new filename.
fn synchronize_filename(path: &Path) -> Result<PathBuf, anyhow::Error> {
    // parse file again to check for synchronicity with filename
    let n = Note::from_existing_note(&path)
        .context("failed to parse YAML front matter: can not synchronize filename!")?;

    println!("Applying template `tmpl_sync_filename`.");
    let new_fqfn = n.render_filename(&CFG.tmpl_sync_filename)?;
    if path != new_fqfn {
        // rename file
        if !Path::new(&new_fqfn).exists() {
            fs::rename(&path, &new_fqfn)?;
            println!("File renamed to {:?}", new_fqfn);
            Ok(new_fqfn)
        } else {
            Err(anyhow!(format!(
                "can not rename file to {:?}\n\
                        (file exists already).\n\
                        Note: at this stage filename and YAML metadata are not in sync!\n\
                        Change `title`/`subtitle` in YAML front matter of file: {:?}
                        ",
                new_fqfn, path
            )))
        }
    } else {
        Ok(path.to_path_buf())
    }
}

#[inline]
/// Create a new note by inserting `tp-note`'s environment in a template.
/// If the note to be created exists already, open it, read the YAML front
/// matter and synchronize the filename if necessary.
fn create_new_note_or_synchronize_filename(path: &Path) -> Result<PathBuf, anyhow::Error> {
    // First generate a new note (if it does not exist), then parse its front_matter
    // and finally rename the file, if it is not in sync with its front matter.
    if path.is_dir() {
        let (n, new_fqfn) = if CLIPBOARD.content.is_empty() {
            // CREATE A NEW NOTE WITH `TMPL_NEW_CONTENT` TEMPLATE
            let n = Note::new(&path, &CFG.tmpl_new_content)
                .context("`can not parse `tmpl_new_content` in config file")?;
            let new_fqfn = n
                .render_filename(&CFG.tmpl_new_filename)
                .context("`can not parse `tmpl_new_filename` in config file")?;
            println!("Applying templates `tmpl_new_content` and `tmpl_new_filename`.");

            (n, new_fqfn)
        } else {
            // CREATE A NEW NOTE WITH `TMPL_CLIPBOARD_CONTENT` TEMPLATE
            let n = Note::new(&path, &CFG.tmpl_clipboard_content)
                .context("`can not parse `tmpl_clipboard_content` in config file")?;
            let new_fqfn = n
                .render_filename(&CFG.tmpl_clipboard_filename)
                .context("`can not parse `tmpl_clipboard_filename` in config file")?;
            println!(
                "Applying templates `tmpl_clipboard_content`, `tmpl_clipboard_filename` \
                and clipboard string: \"{}\"",
                CLIPBOARD.content_truncated
            );
            (n, new_fqfn)
        };

        // Write new note on disk.
        n.write_to_disk(&new_fqfn)
    } else {
        // Is `path` a tp-note file (`.md`) or a foreign file?
        if path
            .extension()
            .unwrap_or_default()
            .to_str()
            .unwrap_or_default()
            == CFG.note_extension.as_str()
        {
            // SYNCHRONIZE FILENAME
            // `path` points to an existing tp-note file.
            // Check if in sync with its filename:
            Ok(synchronize_filename(&path)?)
        } else {
            // ANNOTATE FILE: CREATE NEW NOTE WITH TMPL_ANNOTATE_CONTENT TEMPLATE
            // `path` points to a foreign file type that will be annotated.
            println!("Applying templates `tmpl_annotate_content` and `tmpl_annotate_filename`.");
            let n = Note::new(&path, &CFG.tmpl_annotate_content).with_context(|| {
                format!(
                    "`can not parse `tmpl_annotate_content` in config file: \n'''\n{}\n'''",
                    &CFG.tmpl_annotate_content
                )
            })?;

            let new_fqfn = n.render_filename(&CFG.tmpl_annotate_filename)?;

            // Write new note on disk.
            n.write_to_disk(&new_fqfn)
        }
    }
}

#[inline]
/// Launches some external editor. The editor can be chosen through
/// `tp-note`'s configuration file.
fn launch_editor(path: &Path) -> Result<(), anyhow::Error> {
    // both lists have always the same number of items
    let mut args_list = Vec::new();
    let mut executable_list = Vec::new();

    // prepare launch of editor/viewer
    if ARGS.view {
        for app in &CFG.viewer_args {
            executable_list.push(&app[0]);
            let mut args: Vec<&str> = Vec::new();
            for s in app[1..].iter() {
                args.push(s);
            }
            args.push(
                path.to_str()
                    .ok_or_else(|| anyhow!(format!("failed to convert argument {:?}", path)))?,
            );
            args_list.push(args);
        }
    } else {
        for app in &CFG.editor_args {
            executable_list.push(&app[0]);
            let mut args: Vec<&str> = Vec::new();
            for s in app[1..].iter() {
                args.push(s);
            }
            args.push(
                path.to_str()
                    .ok_or_else(|| anyhow!(format!("failed to convert argument {:?}", path)))?,
            );
            args_list.push(args);
        }
    };

    // launch editor/viewer
    println!("Opening file {:?}", path);

    let mut executable_found = false;
    for i in 0..executable_list.len() {
        let child = Command::new(&executable_list[i])
            .args(&args_list[i])
            .spawn();
        if let Ok(mut child) = child {
            let ecode = child.wait().context("failed to wait on editor to close")?;

            if !ecode.success() {
                return Err(anyhow!("editor did not terminate gracefully"));
            };

            executable_found = true;
            break;
        }
    }

    if !executable_found {
        return Err(anyhow!(format!(
            "No external editor application found in: {:?}",
            &executable_list
        )));
    };

    Ok(())
}

/// High level application algorithm:
/// 1. Create a new note by inserting `tp-note`'s environment in a template.
/// 2. If the note to be created exists already, open it, read the YAML front
///    matter and synchronize the filename if necessary.
/// 3. Open the new note in an external editor (configurable).
/// 4. Read the front matter again and resynchronize the filename if necessary.
#[inline]
fn run() -> Result<(), anyhow::Error> {
    // process arg = `--version`
    if ARGS.version {
        println!("Version {}, {}", VERSION.unwrap_or("unknown"), AUTHOR);
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

    let path = create_new_note_or_synchronize_filename(&path)?;

    // In batch only mode we are done here.
    if ARGS.batch {
        return Ok(());
    };

    launch_editor(&path)?;

    let _path = synchronize_filename(&path)?;

    // Delete clipboard
    if CFG.enable_read_clipboard && CFG.enable_empty_clipboard {
        let ctx: Option<ClipboardContext> = ClipboardProvider::new().ok();
        if let Some(mut ctx) = ctx {
            ctx.set_contents("".to_owned()).unwrap_or_default();
        };
    };

    Ok(())
}

/// Print error message is `run()` does not complete.
fn main() -> Result<(), anyhow::Error> {
    if let Err(e) = run() {
        // Remember the command-line-arguments.
        let mut args_str = String::new();
        for argument in env::args() {
            args_str.push_str(argument.as_str());
            args_str.push(' ');
        }

        if ARGS.batch {
            print_message_console(&format!(
                "Error while executing: {}\n---\n\
                    {:?}\n---",
                args_str, e
            ));
        } else {
            // Unwrap path argument.
            let no_path = PathBuf::new();
            let path: &Path = ARGS.path.as_ref().unwrap_or(&no_path);

            if path.is_file() {
                print_message(&format!(
                    "Error while executing: {}\n---\n\
                    {:?}\n---\nPlease correct error.\n\
                     Trying to start editor without synchronization...",
                    args_str, e
                ));
                launch_editor(path)?;
            } else {
                print_message(&format!(
                    "Error while executing: {}\n---\n\
                    {:?}\n---\nPlease correct error.",
                    args_str, e
                ));
            }
        }
        process::exit(1);
    };
    Ok(())
}
