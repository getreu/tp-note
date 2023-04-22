//! High level program logic implementing the whole workflow.
use crate::config::CFG;
use crate::error::WorkflowError;
use crate::file_editor::launch_editor;
use crate::settings::ARGS;
use crate::settings::CLIPBOARD;
use crate::settings::HTML_EXPORT;
use crate::settings::LAUNCH_EDITOR;
use crate::settings::LAUNCH_VIEWER;
use crate::settings::STDIN;
use crate::template::template_kind_filter;
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
use tpnote_lib::content::ContentString;
use tpnote_lib::error::NoteError;
use tpnote_lib::workflow::create_new_note_or_synchronize_filename;
use tpnote_lib::workflow::synchronize_filename;

/// Run Tp-Note and return the (modified) path to the (new) note file.
/// 1. Create a new note by inserting `Tp-Note`'s environment in a template.
/// 2. If the note to be created exists already, open it, read the YAML front
///    matter and synchronize the filename if necessary.
/// 3. Open the new note in an external editor (configurable).
/// 4. Read the front matter again and resynchronize the filename if necessary.
#[inline]
pub fn run_workflow(mut path: PathBuf) -> Result<PathBuf, WorkflowError> {
    // Depending on this we might not show the viewer later or
    // log an error as WARN level instead of ERROR level.
    let launch_viewer;

    match create_new_note_or_synchronize_filename::<ContentString, _>(
        &path,
        &*CLIPBOARD,
        &*STDIN,
        template_kind_filter,
        &HTML_EXPORT,
        ARGS.force_lang.clone(),
    ) {
        // Use the new `path` from now on.
        Ok(p) => {
            path = p;
            #[cfg(feature = "viewer")]
            {
                launch_viewer = *LAUNCH_VIEWER;
            }
        }
        Err(e) => {
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
                return Err(e.into());
            }
        }
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
        match synchronize_filename::<ContentString>(&path) {
            // `path` has changed!
            Ok(p) => path = p,
            Err(e) => {
                let missing_header = matches!(e, NoteError::MissingFrontMatter { .. })
                    || matches!(e, NoteError::MissingFrontMatterField { .. });

                if missing_header {
                    // Silently ignore error.
                    log::warn!("{}", e);
                } else {
                    // Report all other errors.
                    return Err(e.into());
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

pub(crate) fn run() -> Result<PathBuf, WorkflowError> {
    // process arg = <path>
    let path = if let Some(p) = &ARGS.path {
        p.canonicalize()?
    } else {
        env::current_dir()?
    };
    run_workflow(path)
}
