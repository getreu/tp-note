//! High level program logic implementing the whole workflow.
use crate::config::CFG;
use crate::config2::TMPL_VAR_CLIPBOARD;
use crate::config2::TMPL_VAR_CLIPBOARD_HEADER;
use crate::config2::TMPL_VAR_FM_;
use crate::config2::TMPL_VAR_FM_FILENAME_SYNC;
use crate::config2::TMPL_VAR_FM_NO_FILENAME_SYNC;
use crate::config2::TMPL_VAR_STDIN;
use crate::config2::TMPL_VAR_STDIN_HEADER;
use crate::context::Context;
use crate::error::WorkflowError;
use crate::error2::NoteError;
use crate::file_editor::launch_editor;
use crate::filename;
use crate::filename::MarkupLanguage;
use crate::note::Note;
use crate::settings::ARGS;
use crate::settings::CLIPBOARD;
use crate::settings::LAUNCH_EDITOR;
use crate::settings::LAUNCH_VIEWER;
use crate::settings::STDIN;
#[cfg(feature = "viewer")]
use crate::viewer::launch_viewer_thread;
use std::env;
use std::fs;
#[cfg(not(target_family = "windows"))]
use std::matches;
use std::path::PathBuf;
#[cfg(feature = "viewer")]
use std::thread;
#[cfg(feature = "viewer")]
use std::time::Duration;
use tera::Value;

/// Open the note file `path` on disk and read its YAML front matter.
/// Then calculate from the front matter how the filename should be to
/// be in sync. If it is different, rename the note on disk and return
/// the new filename.
fn synchronize_filename(context: Context) -> Result<PathBuf, WorkflowError> {
    // parse file again to check for synchronicity with filename
    let mut n = Note::from_existing_note(context)?;

    let no_filename_sync = match (
        n.context.get(TMPL_VAR_FM_FILENAME_SYNC),
        n.context.get(TMPL_VAR_FM_NO_FILENAME_SYNC),
    ) {
        // By default we sync.
        (None, None) => false,
        (None, Some(Value::Bool(nsync))) => *nsync,
        (None, Some(_)) => true,
        (Some(Value::Bool(sync)), None) => !*sync,
        _ => false,
    };

    if no_filename_sync {
        log::trace!(
            "Filename synchronisation disabled with the front matter field: `{}: {}`",
            TMPL_VAR_FM_FILENAME_SYNC.trim_start_matches(TMPL_VAR_FM_),
            !no_filename_sync
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

            if !filename::exclude_copy_counter_eq(&n.context.path, &new_file_path) {
                let new_file_path = filename::find_unused(new_file_path)?;

                // rename file
                fs::rename(&n.context.path, &new_file_path)?;
                log::trace!("File renamed to {:?}", new_file_path);
                new_file_path
            } else {
                n.context.path.clone()
            }
        } else {
            n.context.path.clone()
        };

    // Print HTML rendition.
    if let Some(dir) = &ARGS.export {
        n.render_and_write_content(&new_file_path, &CFG.html_tmpl.exporter_tmpl, dir)
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
fn create_new_note_or_synchronize_filename(context: Context) -> Result<PathBuf, WorkflowError> {
    // First generate a new note (if it does not exist), then parse its front_matter
    // and finally rename the file, if it is not in sync with its front matter.
    // Does the first positional parameter point to a directory?
    if context.path == context.dir_path {
        // Yes: create a new note.

        // Error if we are supposed to export a directory.
        if ARGS.export.is_some() {
            return Err(WorkflowError::ExportNeedsNoteFile);
        };

        let (n, new_file_path) = if STDIN.is_empty() && CLIPBOARD.is_empty() {
            // CREATE A NEW NOTE WITH `TMPL_NEW_CONTENT` TEMPLATE
            log::trace!("Applying templates `[tmpl] new_content` and `[tmpl] new_filename`.");
            let n = Note::from_content_template(context, &CFG.tmpl.new_content).map_err(|e| {
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
        // The first positional parameter points to an existing file.
        } else if !STDIN.borrow_dependent().header.is_empty()
            || !CLIPBOARD.borrow_dependent().header.is_empty()
        {
            // There is a valid YAML front matter in the `CLIPBOARD` or `STDIN`.
            // Create a new note based on clipboard or input stream.
            log::trace!("Applying templates: `[tmpl] from_clipboard_yaml_content`, `[tmpl] from_clipboard_yaml_filename`");
            let n = Note::from_content_template(context, &CFG.tmpl.from_clipboard_yaml_content)
                .map_err(|e| WorkflowError::Template {
                    tmpl_name: "[tmpl] from_clipboard_yaml_content".to_string(),
                    source: e,
                })?;

            let new_file_path = n
                .render_filename(&CFG.tmpl.from_clipboard_yaml_filename)
                .map_err(|e| WorkflowError::Template {
                    tmpl_name: "[tmpl] from_clipboard_yaml_filename".to_string(),
                    source: e,
                })?;
            (n, new_file_path)
        } else {
            // Create a new note based on clipboard or input stream without header.
            log::trace!(
                "Applying templates: `[tmpl] from_clipboard_content`, `[tmpl] from_clipboard_filename`"
            );
            let n = Note::from_content_template(context, &CFG.tmpl.from_clipboard_content)
                .map_err(|e| WorkflowError::Template {
                    tmpl_name: "[tmpl] from_clipboard_content".to_string(),
                    source: e,
                })?;

            let new_file_path = n
                .render_filename(&CFG.tmpl.from_clipboard_filename)
                .map_err(|e| WorkflowError::Template {
                    tmpl_name: "[tmpl] from_clipboard_filename".to_string(),
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
        // The first positional paramter `path` points to a file.

        let extension_is_known = !matches!(
            MarkupLanguage::from(
                context
                    .path
                    .extension()
                    .unwrap_or_default()
                    .to_str()
                    .unwrap_or_default()
            ),
            MarkupLanguage::None
        );

        if extension_is_known {
            // SYNCHRONIZE FILENAME
            // `path` points to an existing Tp-Note file.
            // Check if in sync with its filename.
            // If the note file has no header, we prepend one if wished for.
            match (
                synchronize_filename(context.clone()),
                // Shall we prepend a header, in case it is missing?
                (ARGS.add_header || CFG.arg_default.add_header)
                    && !CFG.arg_default.no_filename_sync
                    && !ARGS.no_filename_sync,
            ) {
                (Ok(path), _) => Ok(path),
                (
                    Err(WorkflowError::Note {
                        source: NoteError::MissingFrontMatter { .. },
                    }),
                    true,
                ) => {
                    log::trace!(
                       "Applying template: `[tmpl] from_text_file_content`, `[tmpl] from_text_file_filename`"
                    );
                    let n = Note::from_text_file(context, &CFG.tmpl.from_text_file_content)?;
                    let new_file_path = n
                        .render_filename(&CFG.tmpl.from_text_file_filename)
                        .map_err(|e| WorkflowError::Template {
                            tmpl_name: "[tmpl] from_text_file_filename".to_string(),
                            source: e,
                        })?;
                    n.content.write_to_disk(&*new_file_path)?;
                    if n.context.path != new_file_path {
                        log::trace!("Deleting file: {:?}", n.context.path);
                        fs::remove_file(&n.context.path)?;
                    }
                    Ok(new_file_path)
                }
                (Err(e), _) => Err(e),
            }
        } else {
            // Error if we are supposed to export an unknown file type.
            if ARGS.export.is_some() {
                return Err(WorkflowError::ExportNeedsNoteFile);
            };

            // ANNOTATE FILE: CREATE NEW NOTE WITH TMPL_ANNOTATE_CONTENT TEMPLATE
            // `path` points to a foreign file type that will be annotated.
            log::trace!(
                "Applying templates `[tmpl] annotate_file_content` and `[tmpl] annotate_file_filename`."
            );
            let n = Note::from_content_template(context, &CFG.tmpl.annotate_file_content).map_err(
                |e| WorkflowError::Template {
                    tmpl_name: "[tmpl] annotate_file_content".to_string(),
                    source: e,
                },
            )?;

            let new_file_path = n
                .render_filename(&CFG.tmpl.annotate_file_filename)
                .map_err(|e| WorkflowError::Template {
                    tmpl_name: "[tmpl] annotate_file_filename".to_string(),
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
    // process arg = <path>
    let mut path = if let Some(p) = &ARGS.path {
        p.canonicalize()?
    } else {
        env::current_dir()?
    };

    // Collect input data for templates.
    let mut context = Context::from(&path);
    context.insert_environment()?;
    context.insert_content(TMPL_VAR_CLIPBOARD, TMPL_VAR_CLIPBOARD_HEADER, &CLIPBOARD)?;
    context.insert_content(TMPL_VAR_STDIN, TMPL_VAR_STDIN_HEADER, &STDIN)?;

    // Depending on this we might not show the viewer later or
    // log an error as WARN level instead of ERROR level.
    let launch_viewer;

    match create_new_note_or_synchronize_filename(context) {
        // Use the new `path` from now on.
        Ok(p) => {
            path = p;
            #[cfg(feature = "viewer")]
            {
                launch_viewer = *LAUNCH_VIEWER;
            }
        }
        Err(WorkflowError::Note { source: e }) => {
            if (matches!(e, NoteError::InvalidFrontMatterYaml { .. })
                || matches!(e, NoteError::MissingFrontMatter { .. })
                || matches!(e, NoteError::MissingFrontMatterField { .. })
                || matches!(e, NoteError::CompulsoryFrontMatterFieldIsEmpty { .. })
                || matches!(e, NoteError::SortTagVarInvalidChar { .. })
                || matches!(e, NoteError::FileExtNotRegistered { .. }))
                && !ARGS.batch
                && ARGS.export.is_none()
            {
                // Continue the workflow.

                let missing_header = matches!(e, NoteError::MissingFrontMatter { .. })
                    || matches!(e, NoteError::MissingFrontMatterField { .. });

                launch_viewer = *LAUNCH_VIEWER
                    && !(missing_header
                        && CFG.viewer.missing_header_disables
                        && !CFG.arg_default.add_header
                        && !ARGS.add_header
                        && !ARGS.view);

                if launch_viewer || missing_header {
                    // Inform user when `--debug warn`, then continue workflow.
                    log::warn!("{}", e,);
                } else {
                    // Inform user, then continue workflow.
                    log::error!("{}", e,);
                };
            } else {
                // This is a fatal error, so we quit.
                return Err(WorkflowError::Note { source: e });
            }
        }
        // This is a fatal error, so we quit.
        Err(e) => return Err(e),
    };

    #[cfg(feature = "viewer")]
    let viewer_join_handle = if launch_viewer {
        Some(launch_viewer_thread(&path))
    } else {
        None
    };

    if *LAUNCH_EDITOR {
        #[cfg(feature = "viewer")]
        if viewer_join_handle.is_some() && CFG.viewer.startup_delay < 0 {
            thread::sleep(Duration::from_millis(
                CFG.viewer.startup_delay.unsigned_abs() as u64,
            ));
        };

        // This blocks.
        launch_editor(&path)?;
    };

    if *LAUNCH_EDITOR {
        // Collect input data for templates.
        let mut context = Context::from(&path);
        context.insert_environment()?;

        match synchronize_filename(context) {
            // `path` has changed!
            Ok(p) => path = p,
            Err(e) => {
                let missing_header = matches!(
                    e,
                    WorkflowError::Note {
                        source: NoteError::MissingFrontMatter { .. }
                    }
                ) || matches!(
                    e,
                    WorkflowError::Note {
                        source: NoteError::MissingFrontMatterField { .. }
                    }
                );

                if missing_header {
                    // Silently ignore error.
                    log::warn!("{}", e);
                } else {
                    // Report all other errors.
                    return Err(e);
                }
            }
        };
    } else {
        #[cfg(feature = "viewer")]
        if let Some(jh) = viewer_join_handle {
            let _ = jh.join();
        };
    };

    Ok(path)
}
