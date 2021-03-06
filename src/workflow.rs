//! High level program logic implementing the whole workflow.

use crate::config::CFG;
use crate::error::NoteError;
use crate::error::WorkflowError;
use crate::file_editor::launch_editor;
use crate::filename;
use crate::filename::MarkupLanguage;
use crate::note::Note;
use crate::note::TMPL_VAR_FM_;
use crate::note::TMPL_VAR_FM_NO_FILENAME_SYNC;
use crate::settings::ARGS;
use crate::settings::CLIPBOARD;
use crate::settings::LAUNCH_EDITOR;
use crate::settings::LAUNCH_VIEWER;
#[cfg(feature = "read-clipboard")]
use crate::settings::RUNS_ON_CONSOLE;
use crate::settings::STDIN;
#[cfg(feature = "viewer")]
use crate::viewer::launch_viewer_thread;
use crate::AUTHOR;
use crate::VERSION;
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
use tera::Value;

/// Open the note file `path` on disk and reads its YAML front matter.
/// Then calculate from the front matter how the filename should be to
/// be in sync. If it is different, rename the note on disk and return
/// the new filename.
fn synchronize_filename(path: &Path) -> Result<PathBuf, WorkflowError> {
    // parse file again to check for synchronicity with filename
    let mut n = match Note::from_existing_note(&path) {
        Ok(n) => n,
        Err(e) if matches!(e, NoteError::MissingFrontMatter { .. }) => {
            return Err(WorkflowError::MissingFrontMatter { source: e })
        }
        Err(e) if matches!(e, NoteError::MissingFrontMatterField { .. }) => {
            return Err(WorkflowError::MissingFrontMatterField { source: e })
        }
        Err(e) => return Err(e.into()),
    };

    let no_filename_sync = match n.context.get(TMPL_VAR_FM_NO_FILENAME_SYNC) {
        // By default we sync.
        None => false,
        Some(Value::Bool(nsync)) => *nsync,
        Some(_) => true,
    };

    if no_filename_sync {
        log::trace!(
            "Filename synchronisation disabled with the front matter field: `{}: {}`",
            TMPL_VAR_FM_NO_FILENAME_SYNC.trim_start_matches(TMPL_VAR_FM_),
            no_filename_sync
        );
    }

    if ARGS.no_filename_sync {
        log::trace!("Filename synchronisation disabled with the flag: `--no-filename-sync`",);
    }

    if CFG.arg_default.no_filename_sync {
        log::trace!(
            "Filename synchronisation disabled with the configuration file \
             variable: `[arg_default] no_filename_sync = true`",
        );
    }

    let new_file_path =
        // Do not sync, if explicitly disabled.
        if !no_filename_sync && !CFG.arg_default.no_filename_sync && !ARGS.no_filename_sync {
            log::trace!("Applying template `[tmpl] sync_filename`.");
            let new_file_path = n.render_filename(&CFG.tmpl.sync_filename).map_err(|e| {
                WorkflowError::Template {
                    tmpl_name: "[tmpl] sync_filename".to_string(),
                    source: e,
                }
            })?;

            if !filename::exclude_copy_counter_eq(&path, &new_file_path) {
                let new_file_path = filename::find_unused(new_file_path)?;

                // rename file
                fs::rename(&path, &new_file_path)?;
                log::trace!("File renamed to {:?}", new_file_path);
                new_file_path
            } else {
                path.to_path_buf()
            }
        } else {
            path.to_path_buf()
        };

    // Print HTML rendition.
    if let Some(dir) = &ARGS.export {
        n.render_and_write_content(&new_file_path, &CFG.exporter.rendition_tmpl, &dir)
            .map_err(|e| WorkflowError::Template {
                tmpl_name: "[exporter] rendition_tmpl".to_string(),
                source: e,
            })?;
    }

    Ok(new_file_path)
}

#[inline]
/// Create a new note by inserting `Tp-Note`'s environment in a template.
/// If the note to be created exists already, append a so called `copy_counter`
/// to the filename and try to save it again. In case this does not succeed either,
/// increment the `copy_counter` until a free filename is found.
fn create_new_note_or_synchronize_filename(path: &Path) -> Result<PathBuf, WorkflowError> {
    // First generate a new note (if it does not exist), then parse its front_matter
    // and finally rename the file, if it is not in sync with its front matter.
    if path.is_dir() {
        let (n, new_file_path) = if STDIN.is_empty() && CLIPBOARD.is_empty() {
            // CREATE A NEW NOTE WITH `TMPL_NEW_CONTENT` TEMPLATE
            log::trace!("Applying templates `[tmpl] new_content` and `[tmpl] new_filename`.");
            let n = Note::from_content_template(&path, &CFG.tmpl.new_content).map_err(|e| {
                WorkflowError::Template {
                    tmpl_name: "[tmpl] new_content".to_string(),
                    source: e,
                }
            })?;
            let new_file_path =
                n.render_filename(&CFG.tmpl.new_filename)
                    .map_err(|e| WorkflowError::Template {
                        tmpl_name: "[tmpl] new_filename".to_string(),
                        source: e,
                    })?;
            (n, new_file_path)
        } else if !STDIN.borrow_dependent().header.is_empty()
            || !CLIPBOARD.borrow_dependent().header.is_empty()
        {
            // CREATE A NEW NOTE BASED ON CLIPBOARD OR INPUT STREAM
            // (only if there is a valid YAML front matter)
            log::trace!("Applying templates: `[tmpl] copy_content`, `[tmpl] copy_filename`");
            let n = Note::from_content_template(&path, &CFG.tmpl.copy_content).map_err(|e| {
                WorkflowError::Template {
                    tmpl_name: "[tmpl] copy_content".to_string(),
                    source: e,
                }
            })?;
            // CREATE A NEW NOTE WITH `TMPL_COPY_CONTENT` TEMPLATE
            let new_file_path = n.render_filename(&CFG.tmpl.copy_filename).map_err(|e| {
                WorkflowError::Template {
                    tmpl_name: "[tmpl] copy_filename".to_string(),
                    source: e,
                }
            })?;
            (n, new_file_path)
        } else {
            // CREATE A NEW NOTE BASED ON CLIPBOARD OR INPUT STREAM
            log::trace!(
                "Applying templates: `[tmpl] clipboard_content`, `[tmpl] clipboard_filename`"
            );
            let n =
                Note::from_content_template(&path, &CFG.tmpl.clipboard_content).map_err(|e| {
                    WorkflowError::Template {
                        tmpl_name: "[tmpl] clipboard_content".to_string(),
                        source: e,
                    }
                })?;

            // CREATE A NEW NOTE WITH `TMPL_CLIPBOARD_CONTENT` TEMPLATE
            let new_file_path = n
                .render_filename(&CFG.tmpl.clipboard_filename)
                .map_err(|e| WorkflowError::Template {
                    tmpl_name: "[tmpl] clipboard_filename".to_string(),
                    source: e,
                })?;
            (n, new_file_path)
        };

        // Check if the filename is not taken already
        let new_file_path = filename::find_unused(new_file_path)?;

        // Write new note on disk.
        n.content.write_to_disk(&new_file_path)?;

        Ok(new_file_path)
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
            // `path` points to an existing Tp-Note file.
            // Check if in sync with its filename:
            Ok(synchronize_filename(path)?)
        } else {
            // ANNOTATE FILE: CREATE NEW NOTE WITH TMPL_ANNOTATE_CONTENT TEMPLATE
            // `path` points to a foreign file type that will be annotated.
            log::trace!(
                "Applying templates `[tmpl] annotate_content` and `[tmpl] annotate_filename`."
            );
            let n =
                Note::from_content_template(&path, &CFG.tmpl.annotate_content).map_err(|e| {
                    WorkflowError::Template {
                        tmpl_name: "[tmpl] annotate_content".to_string(),
                        source: e,
                    }
                })?;

            let new_file_path = n
                .render_filename(&CFG.tmpl.annotate_filename)
                .map_err(|e| WorkflowError::Template {
                    tmpl_name: "[tmpl] annotate_filename".to_string(),
                    source: e,
                })?;

            // Check if the filename is not taken already
            let new_file_path = filename::find_unused(new_file_path)?;

            // Write new note on disk.
            n.content.write_to_disk(&new_file_path)?;

            Ok(new_file_path)
        }
    }
}

/// Run Tp-Note and return the (modified) path to the (new) note file.
/// 1. Create a new note by inserting `Tp-Note`'s environment in a template.
/// 2. If the note to be created exists already, open it, read the YAML front
///    matter and synchronize the filename if necessary.
/// 3. Open the new note in an external editor (configurable).
/// 4. Read the front matter again and resynchronize the filename if necessary.
#[inline]
pub fn run() -> Result<PathBuf, WorkflowError> {
    // process arg = `--version`
    if ARGS.version {
        print!("Version {}, {}, ", VERSION.unwrap_or("unknown"), AUTHOR);
        print!("compiled-in features: [");
        #[cfg(feature = "message-box")]
        print!("message-box, ");
        #[cfg(feature = "viewer")]
        print!("viewer, ");
        #[cfg(feature = "renderer")]
        print!("renderer, ");
        #[cfg(feature = "clipboard")]
        print!("clipboard, ");
        println!("]");
        process::exit(0);
    };

    // process arg = <path>
    let mut path = if let Some(p) = &ARGS.path {
        p.canonicalize()?
    } else {
        env::current_dir()?
    };

    if ARGS.export.is_some() && !path.is_file() {
        return Err(WorkflowError::ExportsNeedsNoteFile);
    };

    // Depending on this we might not show the viewer later or
    // log an error as WARN level instead of ERROR level.
    let mut missing_header;

    match create_new_note_or_synchronize_filename(&path) {
        // Use the new `path` from now on.
        Ok(p) => {
            path = p;
            #[cfg(feature = "viewer")]
            {
                missing_header = false;
            }
        }
        Err(e) => {
            if path.is_file()
                && !matches!(e, WorkflowError::Io { .. })
                && !matches!(e, WorkflowError::File { .. })
                && !matches!(e, WorkflowError::Template { .. })
                && !ARGS.batch
            {
                missing_header = matches!(e, WorkflowError::MissingFrontMatter { .. })
                    || matches!(e, WorkflowError::MissingFrontMatterField { .. });

                if *LAUNCH_VIEWER || (missing_header && CFG.silently_ignore_missing_header) {
                    log::warn!(
                        "{}\n\
                        \n\
                        Please correct the front matter if this is supposed \
                        to be a Tp-Note file. Ignore otherwise.",
                        e,
                    );
                } else {
                    log::error!(
                        "{}\n\
                        \n\
                        Please correct the error.",
                        e,
                    );
                };
            } else {
                // If `path` points to a directory, no viewer and no editor can open.
                // This is a fatal error, so we quit.
                return Err(e);
            }
        }
    };

    #[cfg(feature = "viewer")]
    let viewer_join_handle = if *LAUNCH_VIEWER
        && !(missing_header && CFG.viewer.missing_header_disables && !ARGS.view)
    {
        Some(launch_viewer_thread(&path))
    } else {
        None
    };

    if *LAUNCH_EDITOR {
        // This blocks.
        launch_editor(&path)?;
    };

    if *LAUNCH_EDITOR {
        match synchronize_filename(&path) {
            // `path` has changed!
            Ok(p) => path = p,
            Err(e) => {
                missing_header = matches!(e, WorkflowError::MissingFrontMatter { .. })
                    || matches!(e, WorkflowError::MissingFrontMatterField { .. });

                if missing_header && CFG.silently_ignore_missing_header {
                    // Silently ignore error.
                    log::warn!(
                        "{}\n\
                        \n\
                        Please correct the front matter if this is supposed \
                        to be a Tp-Note file. Ignore otherwise.",
                        e,
                    );
                } else {
                    // Report all other errors.
                    return Err(e);
                }
            }
        };

        // Delete clipboard content.
        #[cfg(feature = "read-clipboard")]
        if CFG.clipboard.read_enabled && CFG.clipboard.empty_enabled && !*RUNS_ON_CONSOLE {
            let ctx: Option<ClipboardContext> = ClipboardProvider::new().ok();
            if let Some(mut ctx) = ctx {
                ctx.set_contents("".to_owned()).unwrap_or_default();
            };
        }
    } else {
        #[cfg(feature = "viewer")]
        if let Some(jh) = viewer_join_handle {
            let _ = jh.join();
        };
    };

    Ok(path)
}
