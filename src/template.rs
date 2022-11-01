//! Helper functions to determine the right `TemplateKind` when Tp-Note
//! starts.

use crate::config::CFG;
use crate::settings::ARGS;
use crate::settings::CLIPBOARD;
use crate::settings::STDIN;
use std::path::Path;
use tpnote_lib::content::Content;
use tpnote_lib::filename::NotePath;
use tpnote_lib::template::TemplateKind;

/// `path` is the first positional command line parameter given to Tp-Note.
/// Returns the template that will be used in the further workflow.
/// If `path` points to an existing Tp-Note file (with or without header),
/// `Some<Content>` is the content to the file.
pub(crate) fn get_template_and_content<T: Content>(path: &Path) -> (TemplateKind, Option<T>) {
    let stdin_is_empty = STDIN.is_empty();
    let stdin_has_header = !STDIN.header().is_empty();

    let clipboard_is_empty = CLIPBOARD.is_empty();
    let clipboard_has_header = !CLIPBOARD.header().is_empty();

    let input_stream_is_some = !stdin_is_empty || !clipboard_is_empty;
    let input_stream_has_header = stdin_has_header || clipboard_has_header;

    let path_is_dir = path.is_dir();
    let path_is_file = path.is_file();

    let path_has_tpnote_extension = path.has_tpnote_extension();
    let path_is_tpnote_file = path_is_file && path_has_tpnote_extension;

    let (path_is_tpnote_file_and_has_header, content) = if path_is_tpnote_file {
        let content: T = Content::open(path).unwrap_or_default();
        (!content.header().is_empty(), Some(content))
    } else {
        (false, None)
    };

    // This determines the workflow and what template will be applied.
    let template_kind = match (
        path_is_dir,
        input_stream_is_some,
        input_stream_has_header,
        path_is_file,
        path_is_tpnote_file,
        path_is_tpnote_file_and_has_header,
    ) {
        (true, false, _, false, _, _) => TemplateKind::New,
        (true, true, false, false, _, _) => TemplateKind::FromClipboard,
        (true, true, true, false, _, _) => TemplateKind::FromClipboardYaml,
        (false, _, _, true, true, true) => TemplateKind::SyncFilename,
        (false, _, _, true, true, false) => TemplateKind::FromTextFile,
        (false, _, _, true, false, _) => TemplateKind::AnnotateFile,
        (_, _, _, _, _, _) => TemplateKind::None,
    };

    // Treat inhibitors:
    let template_kind = match template_kind {
        TemplateKind::FromTextFile => {
            if (ARGS.add_header || CFG.arg_default.add_header)
                && !CFG.arg_default.no_filename_sync
                && !ARGS.no_filename_sync
            {
                // No change, we do it.
                template_kind
            } else {
                log::info!(
                    "Not adding header to text file: \
                     `add_header` is not enabled or `no_filename_sync`",
                );
                // We change to `None`.
                TemplateKind::None
            }
        }
        TemplateKind::SyncFilename => {
            if ARGS.no_filename_sync {
                log::info!("Filename synchronisation disabled with the flag: `--no-filename-sync`",);
                TemplateKind::None
            } else if CFG.arg_default.no_filename_sync {
                log::info!(
                    "Filename synchronisation disabled with the configuration file \
             variable: `[arg_default] no_filename_sync = true`",
                );
                TemplateKind::None
            } else {
                // We do it, no change
                template_kind
            }
        }
        // Otherwise, there are no more inhibitors so far.
        _ => template_kind,
    };

    log::debug!("Chosing the \"{:?}\" template.", template_kind);
    log::trace!(
        "Template choice is based on:
        path=\"{}\",
        path_is_dir={},
        input_stream_is_some={},
        input_stream_has_header={},
        path_is_file={},
        path_is_tpnote_file={},
        path_is_tpnote_file_and_has_header={}",
        path.to_str().unwrap(),
        path_is_dir,
        input_stream_is_some,
        input_stream_has_header,
        path_is_file,
        path_is_tpnote_file,
        path_is_tpnote_file_and_has_header,
    );

    (template_kind, content)
}
