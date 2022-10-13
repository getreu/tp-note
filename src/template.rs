//! Helper functions to determine the right `TemplateKind` when Tp-Note
//! starts.

use crate::settings::CLIPBOARD;
use crate::settings::STDIN;
use std::fs;
use std::path::Path;
use tpnote_lib::content::Content;
use tpnote_lib::filename::NotePath;
use tpnote_lib::template::TemplateKind;

/// `path` is the first positional command line parameter given to Tp-Note.
/// Returns the template that will be used in the further workflow.
/// If `path` points to an existing Tp-Note file (with or without header),
/// `Some<Content>` is the content to the file.
#[allow(dead_code)] // TODO
pub(crate) fn get_template_content(path: &Path) -> (TemplateKind, Option<Content>) {
    let stdin_is_empty = STDIN.is_empty();
    let stdin_has_header = !STDIN.borrow_dependent().header.is_empty();

    let clipboard_is_empty = CLIPBOARD.is_empty();
    let clipboard_has_header = !CLIPBOARD.borrow_dependent().header.is_empty();

    let input_stream_is_some = !stdin_is_empty || !clipboard_is_empty;
    let input_stream_has_header = stdin_has_header || clipboard_has_header;

    let path_is_dir = path.is_dir();
    let path_is_file = path.is_file();

    let path_has_tpnote_extension = path.has_tpnote_extension();
    let path_is_tpnote_file = path_is_file && path_has_tpnote_extension;

    let (path_is_tpnote_file_and_has_header, content) = if path_is_tpnote_file {
        let content = Content::from_input_with_cr(fs::read_to_string(path).unwrap_or_default());
        (!content.borrow_dependent().header.is_empty(), Some(content))
    } else {
        (false, None)
    };

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
