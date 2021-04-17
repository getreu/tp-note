//! High level program logic implementing the whole workflow.

use crate::config::ARGS;
use crate::config::CFG;
use crate::config::CLIPBOARD;
use crate::config::LAUNCH_EDITOR;
use crate::config::LAUNCH_VIEWER;
#[cfg(feature = "read-clipboard")]
use crate::config::RUNS_ON_CONSOLE;
use crate::config::STDIN;
use crate::file_editor::launch_editor;
use crate::filename;
use crate::filename::MarkupLanguage;
use crate::note::Note;
#[cfg(feature = "viewer")]
use crate::viewer::launch_viewer_thread;
use crate::AUTHOR;
use crate::VERSION;
use anyhow::{anyhow, Context};
#[cfg(feature = "read-clipboard")]
use clipboard::ClipboardContext;
#[cfg(feature = "read-clipboard")]
use clipboard::ClipboardProvider;
use std::env;
use std::fs;
#[cfg(not(target_family = "windows"))]
use std::matches;
use std::path::Path;
use std::path::PathBuf;
use std::process;

/// Open the note file `path` on disk and reads its YAML front matter.
/// Then calculate from the front matter how the filename should be to
/// be in sync. If it is different, rename the note on disk and return
/// the new filename.
fn synchronize_filename(path: &Path) -> Result<PathBuf, anyhow::Error> {
    // parse file again to check for synchronicity with filename
    let mut n = Note::from_existing_note(&path).context(
        "Failed to parse the note's metadata. \
                  Can not synchronize the note's filename!",
    )?;

    let new_fqfn = if !ARGS.no_sync {
        log::trace!("Applying template `tmpl_sync_filename`.");
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
            log::trace!("File renamed to {:?}", new_fqfn);
            new_fqfn
        } else {
            path.to_path_buf()
        }
    } else {
        path.to_path_buf()
    };

    // Print HTML rendition.
    if let Some(dir) = &ARGS.export {
        n.render_and_write_content(&new_fqfn, &dir)
            .context(format!("Can not write HTML rendition into: {:?}", dir))?;
    }

    Ok(new_fqfn)
}

#[inline]
/// Create a new note by inserting `Tp-Note`'s environment in a template.
/// If the note to be created exists already, append a so called `copy_counter`
/// to the filename and try to save it again. In case this does not succeed either,
/// increment the `copy_counter` until a free filename is found.
fn create_new_note_or_synchronize_filename(path: &Path) -> Result<PathBuf, anyhow::Error> {
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
            log::trace!("Applying templates `tmpl_new_content` and `tmpl_new_filename`.");
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
            log::trace!("Applying templates: `tmpl_copy_content`, `tmpl_copy_filename`");
            (n, new_fqfn)
        } else {
            // CREATE A NEW NOTE BASED ON CLIPBOARD OR INPUT STREAM
            let n = Note::from_content_template(&path, &CFG.tmpl_clipboard_content)
                // CREATE A NEW NOTE WITH `TMPL_CLIPBOARD_CONTENT` TEMPLATE
                .context("Can not render the template `tmpl_clipboard_content` in config file.")?;
            let new_fqfn = n
                .render_filename(&CFG.tmpl_clipboard_filename)
                .context("Can not render the template `tmpl_clipboard_filename` in config file.")?;
            log::trace!("Applying templates: `tmpl_clipboard_content`, `tmpl_clipboard_filename`");
            (n, new_fqfn)
        };

        // Check if the filename is not taken already
        let new_fqfn = filename::find_unused(new_fqfn)?;

        // Write new note on disk.
        n.content.write_to_disk(&new_fqfn)?;

        Ok(new_fqfn)
    } else {
        let extension_is_known = !matches!(
            MarkupLanguage::from(
                None,
                path.extension()
                    .unwrap_or_default()
                    .to_str()
                    .unwrap_or_default()
            ),
            MarkupLanguage::None
        );

        if extension_is_known {
            // SYNCHRONIZE FILENAME
            // `path` points to an existing tp-note file.
            // Check if in sync with its filename:
            Ok(synchronize_filename(path)?)
        } else {
            // ANNOTATE FILE: CREATE NEW NOTE WITH TMPL_ANNOTATE_CONTENT TEMPLATE
            // `path` points to a foreign file type that will be annotated.
            log::trace!("Applying templates `tmpl_annotate_content` and `tmpl_annotate_filename`.");
            let n = Note::from_content_template(&path, &CFG.tmpl_annotate_content)
                .context("Can not render the template `tmpl_annotate_content` in config file.")?;
            let new_fqfn = n
                .render_filename(&CFG.tmpl_annotate_filename)
                .context("Can not render the template `tmpl_annotate_filename` in config file.")?;

            // Check if the filename is not taken already
            let new_fqfn = filename::find_unused(new_fqfn)?;

            // Write new note on disk.
            n.content.write_to_disk(&new_fqfn)?;

            Ok(new_fqfn)
        }
    }
}

/// Run Tp-Note and return the (modified) path to the (new) note file.
/// 1. Create a new note by inserting `tp-note`'s environment in a template.
/// 2. If the note to be created exists already, open it, read the YAML front
///    matter and synchronize the filename if necessary.
/// 3. Open the new note in an external editor (configurable).
/// 4. Read the front matter again and resynchronize the filename if necessary.
#[inline]
pub fn run() -> Result<PathBuf, anyhow::Error> {
    // process arg = `--version`
    if ARGS.version {
        println!("Version {}, {}", VERSION.unwrap_or("unknown"), AUTHOR);
        process::exit(0);
    };

    // process arg = <path>
    let mut path = if let Some(p) = &ARGS.path {
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

    if ARGS.export.is_some() && !path.is_file() {
        return Err(anyhow!(
            "`--export` and `-x` need a <path> to an existing note file.\n\
              *    Current <path>:\n{}",
            path.as_os_str().to_str().unwrap_or_default()
        ));
    };

    match create_new_note_or_synchronize_filename(&path) {
        // Use the new `path` from now on.
        Ok(p) => path = p,
        Err(e) => {
            // When the viewer is launched, we do not need a dialog
            // box to communicate the error to the user.
            // In this case we skip the following.
            if !*LAUNCH_VIEWER && *LAUNCH_EDITOR {
                log::error!(
                    "{:?}\n\
                    \n\
                    Please correct the error.
                    Trying to start the editor without synchronization...",
                    e
                );
            } else if !*LAUNCH_VIEWER || !path.is_file() {
                // If `path` points to a directory, no viewer and no editor can open.
                // This is a fatal error, so we quit.
                return Err(e);
            }
        }
    };

    #[cfg(feature = "viewer")]
    let viewer_join_handle = if *LAUNCH_VIEWER {
        Some(launch_viewer_thread(&path))
    } else {
        None
    };

    if *LAUNCH_EDITOR {
        // This blocks.
        launch_editor(&path)?;
    };

    #[cfg(feature = "viewer")]
    if let Some(jh) = viewer_join_handle {
        jh.join()
            .map_err(|_| anyhow!("can not join the Viewer thread."))?;
    };

    if *LAUNCH_EDITOR {
        match synchronize_filename(&path) {
            Ok(p) => path = p,
            Err(e) => {
                if !*LAUNCH_VIEWER {
                    // As there is no viewer, the uses must be informed about the error.
                    return Err(e);
                } else {
                    // Silently ignore error, but do not delete clipboard.
                    return Ok(path);
                }
            }
        };

        // Delete clipboard content.
        #[cfg(feature = "read-clipboard")]
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
