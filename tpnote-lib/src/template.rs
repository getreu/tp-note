//!Abstractions for content templates and filename templates.
use crate::filename::NotePath;
use crate::settings::SETTINGS;
use crate::{config::LIB_CFG, content::Content};
use std::path::Path;

/// Each workflow is related to one `TemplateKind`, which relates to one
/// content template and one filename template.
#[non_exhaustive]
#[derive(Default, Debug, PartialEq, Eq, Clone, Copy)]
pub enum TemplateKind {
    /// Templates used when Tp-Note is invoked with a directory path.
    FromDir,
    /// Templates used when the clipboard contains a text with a YAML header.
    FromClipboardYaml,
    /// Templates used when the clipboard contains a text without header.
    FromClipboard,
    /// Templates used when Tp-Note is invoked with a path pointing to a text file
    /// that does not contain a YAML header.
    FromTextFile,
    /// Templates used when Tp-Note is invoked with a path pointing to a non text
    /// file.
    AnnotateFile,
    /// Templates used when Tp-Note is invoked with a path pointing to a Tp-Note
    /// text file with a valid YAML header (with a `title:` field).
    SyncFilename,
    /// No templates are used, but the file is still parsed in order to render it
    /// later to HTML (cf. `<Note>.render_content_to_html()` and
    /// `<Note>.export_html()`).
    #[default]
    None,
}

impl TemplateKind {
    /// A constructor returning the tuple `(template_kind, Some(content))`.
    /// `template_kind` is the result of the logic calculating under what
    /// circumstances what template should be used.
    /// If `path` has a Tp-Note extension (e.g. `.md`) and the file indicated by
    /// `path` could be opened and loaded from disk, `Some(content)` contains
    /// its content. Otherwise `None` is returned.
    pub fn from<T: Content>(
        path: &Path,
        html_clipboard: &T,
        txt_clipboard: &T,
        stdin: &T,
    ) -> (Self, Option<T>) {
        let stdin_is_empty = stdin.is_empty();
        let stdin_has_header = !stdin.header().is_empty();

        let clipboard_is_empty = html_clipboard.is_empty() && txt_clipboard.is_empty();
        let clipboard_has_header =
            !html_clipboard.header().is_empty() || !txt_clipboard.header().is_empty();

        let input_stream_is_some = !stdin_is_empty || !clipboard_is_empty;
        let input_stream_has_header = stdin_has_header || clipboard_has_header;

        let path_is_dir = path.is_dir();
        let path_is_file = path.is_file();

        let path_has_tpnote_extension = path.has_tpnote_ext();
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
            (true, false, _, false, _, _) => TemplateKind::FromDir,
            (true, true, false, false, _, _) => TemplateKind::FromClipboard,
            (true, true, true, false, _, _) => TemplateKind::FromClipboardYaml,
            (false, _, _, true, true, true) => TemplateKind::SyncFilename,
            (false, _, _, true, true, false) => TemplateKind::FromTextFile,
            (false, _, _, true, false, _) => TemplateKind::AnnotateFile,
            (_, _, _, _, _, _) => TemplateKind::None,
        };

        log::debug!("Choosing the \"{:?}\" template.", template_kind);

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
    /// Panics for `TemplateKind::SyncFilename` and `TemplateKind::None`.
    pub fn get_content_template(&self) -> String {
        let lib_cfg = LIB_CFG.read_recursive();
        let scheme_idx = SETTINGS.read_recursive().current_scheme;
        log::trace!(
            "Scheme index: {}, applying the content template: `{}`",
            scheme_idx,
            self.get_content_template_name()
        );
        let tmpl = &lib_cfg.scheme[scheme_idx].tmpl;

        match self {
            Self::FromDir => tmpl.from_dir_content.clone(),
            Self::FromClipboardYaml => tmpl.from_clipboard_yaml_content.clone(),
            Self::FromClipboard => tmpl.from_clipboard_content.clone(),
            Self::FromTextFile => tmpl.from_text_file_content.clone(),
            Self::AnnotateFile => tmpl.annotate_file_content.clone(),
            Self::SyncFilename => {
                panic!("`TemplateKind::SyncFilename` has no content template")
            }
            Self::None => panic!("`TemplateKind::None` has no content template"),
        }
    }

    /// Returns the content template variable name as it is used in the configuration file.
    pub fn get_content_template_name(&self) -> &str {
        match self {
            Self::FromDir => "tmpl.from_dir_content",
            Self::FromClipboardYaml => "tmpl.from_clipboard_yaml_content",
            Self::FromClipboard => "tmpl.from_clipboard_content",
            Self::FromTextFile => "tmpl.from_text_file_content",
            Self::AnnotateFile => "tmpl.annotate_file_content",
            Self::SyncFilename => "`TemplateKind::SyncFilename` has no content template",
            Self::None => "`TemplateKind::None` has no content template",
        }
    }

    /// Returns the file template string as it is defined in the configuration file.
    /// Panics for `TemplateKind::None`.
    pub fn get_filename_template(&self) -> String {
        let lib_cfg = LIB_CFG.read_recursive();
        let scheme_idx = SETTINGS.read_recursive().current_scheme;
        log::trace!(
            "Scheme index: {}, applying the filename template: `{}`",
            scheme_idx,
            self.get_filename_template_name()
        );
        let tmpl = &lib_cfg.scheme[scheme_idx].tmpl;

        match self {
            Self::FromDir => tmpl.from_dir_filename.clone(),
            Self::FromClipboardYaml => tmpl.from_clipboard_yaml_filename.clone(),
            Self::FromClipboard => tmpl.from_clipboard_filename.clone(),
            Self::FromTextFile => tmpl.from_text_file_filename.clone(),
            Self::AnnotateFile => tmpl.annotate_file_filename.clone(),
            Self::SyncFilename => tmpl.sync_filename.clone(),
            Self::None => panic!("`TemplateKind::None` has no filename template"),
        }
    }

    /// Returns the content template variable name as it is used in the configuration file.
    pub fn get_filename_template_name(&self) -> &str {
        match self {
            Self::FromDir => "tmpl.from_dir_filename",
            Self::FromClipboardYaml => "tmpl.from_clipboard_yaml_filename",
            Self::FromClipboard => "tmpl.from_clipboard_filename",
            Self::FromTextFile => "tmpl.from_text_file_filename",
            Self::AnnotateFile => "tmpl.annotate_file_filename",
            Self::SyncFilename => "tmpl.sync_filename",
            Self::None => "`TemplateKind::None` has no filename template",
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::content::ContentString;

    use super::*;

    #[test]
    fn test_template_kind_from() {
        use std::env::temp_dir;
        use std::fs;

        // Some data in the clipboard.
        let tk = TemplateKind::from(
            Path::new("."),
            &ContentString::from("my html input".to_string()),
            &ContentString::from("".to_string()),
            &ContentString::from("".to_string()),
        );
        assert_eq!(tk, (TemplateKind::FromClipboard, None));

        // Some data in the clipboard.
        let tk = TemplateKind::from(
            Path::new("."),
            &ContentString::from("".to_string()),
            &ContentString::from("my txt clipboard".to_string()),
            &ContentString::from("".to_string()),
        );
        assert_eq!(tk, (TemplateKind::FromClipboard, None));

        //
        // No data in the clipboard.
        let tk = TemplateKind::from(
            Path::new("."),
            &ContentString::from("".to_string()),
            &ContentString::from("".to_string()),
            &ContentString::from("".to_string()),
        );
        assert_eq!(tk, (TemplateKind::FromDir, None));

        //
        // No data in the clipboard.
        let tk = TemplateKind::from(
            Path::new("."),
            &ContentString::from("<!DOCTYPE html>".to_string()),
            &ContentString::from("".to_string()),
            &ContentString::from("".to_string()),
        );
        assert_eq!(tk, (TemplateKind::FromDir, None));

        //
        // Tp-Note file.
        // Prepare test: open existing text file without header.
        let raw = "Body text without header";
        let notefile = temp_dir().join("no header.md");
        let _ = fs::write(&notefile, raw.as_bytes());
        // Execute test.
        let (tk, content) = TemplateKind::from(
            &notefile,
            &ContentString::from("<!DOCTYPE html>".to_string()),
            &ContentString::from("".to_string()),
            &ContentString::from("".to_string()),
        );
        // Inspect result.
        let expected_template_kind = TemplateKind::FromTextFile;
        let expected_body = "Body text without header";
        let expected_header = "";
        //println!("{:?}", tk);
        assert_eq!(tk, expected_template_kind);
        let content = content.unwrap();
        assert_eq!(content.header(), expected_header);
        assert_eq!(content.body(), expected_body);
        let _ = fs::remove_file(&notefile);

        //
        // Tp-Note file.
        // Prepare test: open existing note file with header.
        let raw = "---\ntitle: my doc\n---\nBody";
        let notefile = temp_dir().join("some.md");
        let _ = fs::write(&notefile, raw.as_bytes());
        // Execute test.
        let (tk, content) = TemplateKind::from(
            &notefile,
            &ContentString::from("<!DOCTYPE html>".to_string()),
            &ContentString::from("".to_string()),
            &ContentString::from("".to_string()),
        );
        // Inspect result.
        let expected_template_kind = TemplateKind::SyncFilename;
        let expected_body = "Body";
        let expected_header = "title: my doc";
        //println!("{:?}", tk);
        assert_eq!(tk, expected_template_kind);
        let content = content.unwrap();
        assert_eq!(content.header(), expected_header);
        assert_eq!(content.body(), expected_body);
        let _ = fs::remove_file(&notefile);

        //
        // Non-Tp-Note file.
        // Prepare test: annotate existing PDF file.
        let raw = "some data";
        let notefile = temp_dir().join("some.pdf");
        let _ = fs::write(&notefile, raw.as_bytes());
        // Execute test.
        let (tk, content) = TemplateKind::from(
            &notefile,
            &ContentString::from("<!DOCTYPE html>".to_string()),
            &ContentString::from("".to_string()),
            &ContentString::from("".to_string()),
        );
        // Inspect result.
        let expected_template_kind = TemplateKind::AnnotateFile;
        assert_eq!(tk, expected_template_kind);
        assert_eq!(content, None);
        let _ = fs::remove_file(&notefile);
    }
}
