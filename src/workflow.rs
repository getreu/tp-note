//! High level program logic implementing the whole workflow.
use crate::config::CFG;
use crate::error::WorkflowError;
use crate::file_editor::launch_editor;
use crate::settings::ARGS;
use crate::settings::CLIPBOARD;
use crate::settings::LAUNCH_EDITOR;
use crate::settings::LAUNCH_VIEWER;
use crate::settings::STDIN;
#[cfg(feature = "viewer")]
use crate::viewer::launch_viewer_thread;
use std::env;
#[cfg(not(target_family = "windows"))]
use std::matches;
use std::path::PathBuf;
#[cfg(feature = "viewer")]
use std::thread;
#[cfg(feature = "viewer")]
use std::time::Duration;
use tera::Value;
use tpnote_lib::config::TMPL_VAR_CLIPBOARD;
use tpnote_lib::config::TMPL_VAR_CLIPBOARD_HEADER;
use tpnote_lib::config::TMPL_VAR_FM_;
use tpnote_lib::config::TMPL_VAR_FM_FILENAME_SYNC;
use tpnote_lib::config::TMPL_VAR_FM_NO_FILENAME_SYNC;
use tpnote_lib::config::TMPL_VAR_STDIN;
use tpnote_lib::config::TMPL_VAR_STDIN_HEADER;
use tpnote_lib::content::Content;
use tpnote_lib::context::Context;
use tpnote_lib::error::NoteError;
use tpnote_lib::note::Note;
use tpnote_lib::template::TemplateKind;

/// Open the note file `path` on disk and read its YAML front matter.
/// Then calculate from the front matter how the filename should be to
/// be in sync. If it is different, rename the note on disk and return
/// the new filename in `note.rendered_filename`.
/// If no filename was rendered, `note.rendered_filename == PathBuf::new()`
fn synchronize_filename(context: Context, content: Option<Content>) -> Result<Note, WorkflowError> {
    // parse file again to check for synchronicity with filename
    let mut n = Note::from_text_file(context, content, TemplateKind::SyncFilename)?;

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
        log::info!(
            "Filename synchronisation disabled with the front matter field: `{}: {}`",
            TMPL_VAR_FM_FILENAME_SYNC.trim_start_matches(TMPL_VAR_FM_),
            !no_filename_sync
        );
    } else {
        n.render_filename(TemplateKind::SyncFilename)?;

        // Silently fails is source and target are identical.
        n.rename_file_from(&n.context.path)?;
    }

    Ok(n)
}

#[inline]
/// Create a new note by inserting `Tp-Note`'s environment in a template.
/// If the note to be created exists already, append a so called `copy_counter`
/// to the filename and try to save it again. In case this does not succeed either,
/// increment the `copy_counter` until a free filename is found.
/// The return path points to the (new) note file on disk.
/// If an exisiting note file was not moved, the return path equals to `context.path`.
fn create_new_note_or_synchronize_filename(context: Context) -> Result<PathBuf, WorkflowError> {
    // `template_type` will tell us what to do.
    let (template_kind, content) = crate::template::get_template_content(&context.path);
    // First generate a new note (if it does not exist), then parse its front_matter
    // and finally rename the file, if it is not in sync with its front matter.
    // Does the first positional parameter point to a directory?

    let n = match template_kind {
        TemplateKind::New
        | TemplateKind::FromClipboardYaml
        | TemplateKind::FromClipboard
        | TemplateKind::AnnotateFile => {
            // CREATE A NEW NOTE WITH `TMPL_NEW_CONTENT` TEMPLATE
            let mut n = Note::from_content_template(context, template_kind)?;
            n.render_filename(template_kind)?;
            // Check if the filename is not taken already
            n.set_next_unused_rendered_filename()?;
            n.save()?;
            n
        }

        TemplateKind::FromTextFile => {
            let mut n = Note::from_text_file(context, content, template_kind)?;
            // Render filename.
            n.render_filename(template_kind)?;

            // Save new note.
            let context_path = n.context.path.clone();
            n.save_and_delete_from(&context_path)?;
            n
        }
        TemplateKind::SyncFilename => synchronize_filename(context, content)?,
        TemplateKind::None =>
        // Early return, we do nothing here and continue.
        {
            return Ok(context.path)
        }
    };

    // Export HTML rendition, if wanted.
    if let Some(dir) = &ARGS.export {
        n.export_html(&CFG.html_tmpl.exporter_tmpl, dir)?;
    }

    // If no new filename was rendered, return the old one.
    if n.rendered_filename != PathBuf::new() {
        Ok(n.rendered_filename)
    } else {
        Ok(n.context.path)
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

        match synchronize_filename(context, None) {
            // `path` has changed!
            Ok(n) => path = n.rendered_filename,
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
