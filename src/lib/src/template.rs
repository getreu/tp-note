//!Abstractions for content templates and filename templates.
use crate::filename::NotePath;
use crate::{config::LIB_CFG, content::Content};
use std::path::Path;

/// Each workflow is related to one `TemplateKind`, which relates to one
/// content template and one filename template.
#[non_exhaustive]
#[derive(Default, Debug, PartialEq, Eq, Clone, Copy)]
pub enum TemplateKind {
    /// Templates used when Tp-Note is invoked with a directory path.
    New,
    /// Templates used when the clipboard contains a text with a YAML header.
    FromClipboardYaml,
    /// Templates used when the clipboard contains a text without header.
    FromClipboard,
    /// Templates used when Tp-Note is invoked with a path pointing to a text file
    /// does not contain a YAML header.
    FromTextFile,
    /// Templates used when Tp-Note is invoked with a path pointing to a non text
    /// file.
    AnnotateFile,
    /// Templates used when Tp-Note is invoked with a path pointing to a Tp-Note
    /// text file with YAML header.
    SyncFilename,
    /// No templates are used, but the file is still parsed in order to render it
    /// later to HTML.
    #[default]
    None,
}

impl TemplateKind {
    /// Constructor encoding the logic under what circumstances what template.
    /// should be used.
    ///
    /// Contract: If `template_kind` is one of:
    /// * `TemplateKind::New`
    /// * `TemplateKind::FromClipboardYaml`
    /// * `TemplateKind::FromClipboard`
    /// * `TemplateKind::AnnotateFile`
    /// `content` is `None` because it is not used. Otherwise the file's content from the
    /// is read from the disk and returned as `Some(..)`.
    pub fn from<T: Content>(path: &Path, clipboard: &T, stdin: &T) -> (Self, Option<T>) {
        let stdin_is_empty = stdin.is_empty();
        let stdin_has_header = !stdin.header().is_empty();

        let clipboard_is_empty = clipboard.is_empty();
        let clipboard_has_header = !clipboard.header().is_empty();

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

    /// Returns the content template string as it is defined in the configuration file.
    pub fn get_content_template(&self) -> String {
        match self {
            Self::New => LIB_CFG.read().unwrap().tmpl.new_content.clone(),
            Self::FromClipboardYaml => LIB_CFG
                .read()
                .unwrap()
                .tmpl
                .from_clipboard_yaml_content
                .clone(),
            Self::FromClipboard => LIB_CFG.read().unwrap().tmpl.from_clipboard_content.clone(),
            Self::FromTextFile => LIB_CFG.read().unwrap().tmpl.from_text_file_content.clone(),
            Self::AnnotateFile => LIB_CFG.read().unwrap().tmpl.annotate_file_content.clone(),
            Self::SyncFilename => String::new(),
            Self::None => String::new(),
        }
    }

    /// Returns the content template variable name as it is used in the configuration file.
    pub fn get_content_template_name(&self) -> &str {
        match self {
            Self::New => "[tmpl] new_content",
            Self::FromClipboardYaml => "[tmpl] from_clipboard_yaml_content",
            Self::FromClipboard => "[tmpl] from_clipboard_content",
            Self::FromTextFile => "[tmpl] from_text_file_content",
            Self::AnnotateFile => "[tmpl] annotate_file_content",
            Self::SyncFilename => "error: there is no `sync_content` template",
            Self::None => "error: no content template should be used",
        }
    }

    /// Returns the file template string as it is defined in the configuration file.
    pub fn get_filename_template(&self) -> String {
        match self {
            Self::New => LIB_CFG.read().unwrap().tmpl.new_filename.clone(),
            Self::FromClipboardYaml => LIB_CFG
                .read()
                .unwrap()
                .tmpl
                .from_clipboard_yaml_filename
                .clone(),
            Self::FromClipboard => LIB_CFG.read().unwrap().tmpl.from_clipboard_filename.clone(),
            Self::FromTextFile => LIB_CFG.read().unwrap().tmpl.from_text_file_filename.clone(),
            Self::AnnotateFile => LIB_CFG.read().unwrap().tmpl.annotate_file_filename.clone(),
            Self::SyncFilename => LIB_CFG.read().unwrap().tmpl.sync_filename.clone(),
            Self::None => String::new(),
        }
    }

    /// Returns the content template variable name as it is used in the configuration file.
    pub fn get_filename_template_name(&self) -> &str {
        match self {
            Self::New => "[tmpl] new_filename",
            Self::FromClipboardYaml => "[tmpl] from_clipboard_yaml_filename",
            Self::FromClipboard => "[tmpl] from_clipboard_filename",
            Self::FromTextFile => "[tmpl] from_text_file_filename",
            Self::AnnotateFile => "[tmpl] annotate_file_filename",
            Self::SyncFilename => "[tmpl] sync_filename",
            Self::None => "error: no filename template defined yet",
        }
    }
}
