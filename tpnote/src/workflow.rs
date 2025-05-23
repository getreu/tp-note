//! High level program logic implementing the whole workflow.
use crate::config::CFG;
use crate::error::WorkflowError;
use crate::file_editor::launch_editor;
use crate::settings::ARGS;
use crate::settings::DOC_PATH;
use crate::settings::LAUNCH_EDITOR;
use crate::settings::LAUNCH_VIEWER;
use crate::settings::STDIN;
use crate::settings::SYSTEM_CLIPBOARD;
use crate::template::template_kind_filter;
#[cfg(feature = "viewer")]
use crate::viewer::launch_viewer_thread;
#[cfg(not(target_family = "windows"))]
use std::matches;
use std::path::PathBuf;
#[cfg(feature = "viewer")]
use std::thread;
#[cfg(feature = "viewer")]
use std::time::Duration;
use tpnote_lib::content::ContentString;
use tpnote_lib::error::NoteError;
use tpnote_lib::workflow::WorkflowBuilder;

/// Run Tp-Note and return the (modified) path to the (new) note file.
/// 1. Create a new note by inserting Tp-Note's environment in a template.
/// 2. If the note to be created exists already, open it, read the YAML front
///    matter and synchronize the filename if necessary.
/// 3. Open the new note in an external editor (configurable).
/// 4. Read the front matter again and resynchronize the filename if necessary.
#[inline]
pub fn run_workflow(mut path: PathBuf) -> Result<PathBuf, WorkflowError> {
    // Depending on this we might not show the viewer later or
    // log an error as WARN level instead of ERROR level.
    let launch_viewer;

    let mut workflow_builder = WorkflowBuilder::new(&path).upgrade::<ContentString, _>(
        &CFG.arg_default.scheme,
        vec![&SYSTEM_CLIPBOARD.html, &SYSTEM_CLIPBOARD.txt, &*STDIN],
        template_kind_filter,
    );
    if let Some(scheme) = ARGS.scheme.as_deref() {
        workflow_builder.force_scheme(scheme);
    }

    if let Some(lang) = ARGS.force_lang.as_deref() {
        if lang == "-" {
            workflow_builder.force_lang("");
        } else {
            workflow_builder.force_lang(lang);
        }
    }

    if let Some(path) = &ARGS.export {
        workflow_builder.html_export(
            path,
            ARGS.export_link_rewriting
                .unwrap_or(CFG.arg_default.export_link_rewriting),
        );
    }

    let workflow = workflow_builder.build();

    match workflow.run() {
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
                || matches!(e, NoteError::FrontMatterFieldIsCompound { .. })
                || matches!(e, NoteError::FrontMatterFieldIsDuplicateSortTag { .. })
                || matches!(e, NoteError::FrontMatterFieldIsEmptyString { .. })
                || matches!(e, NoteError::FrontMatterFieldIsInvalidSortTag { .. })
                || matches!(e, NoteError::FrontMatterFieldIsNotBool { .. }))
                || matches!(e, NoteError::FrontMatterFieldIsNotNumber { .. })
                || matches!(e, NoteError::FrontMatterFieldIsNotString { .. })
                || matches!(e, NoteError::FrontMatterFieldIsNotTpnoteExtension { .. })
                || matches!(e, NoteError::FrontMatterFieldMissing { .. })
                || matches!(e, NoteError::FrontMatterMissing { .. })
                    && !ARGS.batch
                    && ARGS.export.is_none()
            {
                // Continue the workflow.

                let missing_header = matches!(e, NoteError::FrontMatterMissing { .. })
                    || matches!(e, NoteError::FrontMatterFieldMissing { .. });

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
        let workflow = WorkflowBuilder::new(&path).build();
        match workflow.run::<ContentString>() {
            // `path` has changed!
            Ok(p) => path = p,
            Err(e) => {
                let missing_header = matches!(e, NoteError::FrontMatterMissing { .. })
                    || matches!(e, NoteError::FrontMatterFieldMissing { .. });

                if missing_header && *LAUNCH_VIEWER {
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

#[inline]
pub(crate) fn run() -> Result<PathBuf, WorkflowError> {
    // Process arg = <path>
    let doc_path = DOC_PATH.as_deref()?;
    run_workflow(doc_path.to_path_buf())
}
