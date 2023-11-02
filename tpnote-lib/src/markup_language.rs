//! Helper functions dealing with markup languages.
use crate::config::LIB_CFG;
#[cfg(feature = "renderer")]
use crate::error::NoteError;
#[cfg(feature = "renderer")]
use crate::highlight::SyntaxPreprocessor;
use crate::settings::SETTINGS;
use parse_hyperlinks::renderer::text_links2html;
use parse_hyperlinks::renderer::text_rawlinks2html;
#[cfg(feature = "renderer")]
use pulldown_cmark::{html, Options, Parser};
#[cfg(feature = "renderer")]
use rst_parser::parse;
#[cfg(feature = "renderer")]
use rst_renderer::render_html;
use serde::{Deserialize, Serialize};
use std::path::Path;
#[cfg(feature = "renderer")]
use std::str::from_utf8;

/// The Markup language of the note content.
#[derive(Default, Debug, Hash, Clone, Eq, PartialEq, Deserialize, Serialize, Copy)]
pub enum MarkupLanguage {
    Markdown,
    Restructuredtext,
    Html,
    PlainText,
    /// The exporter renders this, but the viewer is disabled.
    PlainTextNoViewer,
    /// This is a Tp-Note file, but we are not able to determine the
    /// MarkupLanguage at this point.
    Unkown,
    /// This is not a Tp-Note file.
    #[default]
    None,
}

impl MarkupLanguage {
    /// If `Self` is `None` return `rhs`, otherwise return `Self`.
    pub fn or(self, rhs: Self) -> Self {
        match self {
            MarkupLanguage::None => rhs,
            _ => self,
        }
    }

    /// Returns the MIME type for all `Markup Languages.is_tpnote_file()==true`.
    /// Otherwise, for `MarkupLanguage::None` this returns None.
    pub fn mine_type(&self) -> Option<&'static str> {
        match self {
            Self::Markdown => Some("text/markodwn"),
            Self::Restructuredtext => Some("x-rst"),
            Self::Html => Some("text/html"),
            Self::PlainText => Some("text/plain"),
            Self::PlainTextNoViewer => Some("text/plain"),
            Self::Unkown => Some("text/plain"),
            _ => None,
        }
    }

    /// As we identify a markup language by the file's extension, we
    /// can also tell, in case `Markuplanguage::from(ext).is_some()`,
    /// that a file with the extension `ext` is a Tp-Note file.
    pub fn is_some(&self) -> bool {
        !matches!(self, Self::None)
    }

    /// As we identify a markup language by the file's extension, we
    /// can also tell, in case `Markuplanguage::from(ext).is_none()`,
    /// that a file with the extension `ext` is NOT a Tp-Note file.
    pub fn is_none(&self) -> bool {
        matches!(self, Self::None)
    }

    /// Every `MarkupLanguage` variant has an own internal HTML renderer:
    /// * `Markdown` is rendered according the "CommonMark" standard.
    /// * Currently only as small subset of ReStructuredText is rendered for
    ///   `Restructuredtext`. This feature is experimental.
    /// * The `Html` renderer simply forwards the input without modification.
    /// * `PlainText` is rendered as raw text. Hyperlinks in Markdown,
    ///   ReStructuredText, AsciiDoc and WikiText syntax are detected and
    ///   are displayed in the rendition with their link text. All hyperlinks
    ///   are clickable.
    /// * `Unknown` is rendered like `PlainText`, hyperlinks are also
    ///   clickable, but they are displayed as they appear in the input.
    /// * For the variant `None` the result is always the empty string whatever
    ///   the input may be.
    pub fn render(&self, input: &str) -> String {
        match self {
            #[cfg(feature = "renderer")]
            Self::Markdown => {
                // Set up options and parser. Besides the CommonMark standard
                // we enable some useful extras.

                let options = Options::all();
                let parser = Parser::new_ext(input, options);
                let parser = SyntaxPreprocessor::new(parser);

                // Write to String buffer.
                let mut html_output: String = String::with_capacity(input.len() * 3 / 2);
                html::push_html(&mut html_output, parser);
                html_output
            }

            #[cfg(feature = "renderer")]
            Self::Restructuredtext => {
                // Note, that the current rst renderer requires files to end with a new line.
                // <https://github.com/flying-sheep/rust-rst/issues/30>
                let mut rest_input = input.trim_start();
                // The rst parser accepts only exactly one newline at the end.
                while rest_input.ends_with("\n\n") {
                    rest_input = &rest_input[..rest_input.len() - 1];
                }
                // Write to String buffer.
                let mut html_output: Vec<u8> = Vec::with_capacity(rest_input.len() * 3 / 2);
                parse(rest_input.trim_start())
                    .and_then(|doc| render_html(&doc, &mut html_output, false))
                    .map_or_else(
                        |e| NoteError::RstParse { msg: e.to_string() }.to_string(),
                        |_| from_utf8(&html_output).unwrap_or_default().to_string(),
                    )
            }

            Self::Html => input.to_string(),

            Self::PlainText | Self::PlainTextNoViewer => text_links2html(input),

            Self::Unkown => text_rawlinks2html(input),

            _ => String::new(),
        }
    }
}

impl From<&Path> for MarkupLanguage {
    /// Is the file extension ` at the end of the given path listed in
    /// `file.extensions`?
    #[inline]
    fn from(path: &Path) -> Self {
        let file_extension = path
            .extension()
            .unwrap_or_default()
            .to_str()
            .unwrap_or_default();

        Self::from(file_extension)
    }
}

impl From<&str> for MarkupLanguage {
    /// Is `file_extension` listed in `file.extensions`?
    #[inline]
    fn from(file_extension: &str) -> Self {
        let scheme = &LIB_CFG.read_recursive().scheme[SETTINGS.read_recursive().scheme_default];

        for e in &scheme.filename.extensions {
            if e.0 == file_extension {
                return e.1;
            }
        }

        // Nothing was found.
        MarkupLanguage::None
    }
}

#[cfg(test)]
mod tests {

    use crate::markup_language::MarkupLanguage;
    use std::path::Path;

    #[test]
    fn test_markuplanguage_from() {
        //
        let path = Path::new("/dir/file.md");
        assert_eq!(MarkupLanguage::from(path), MarkupLanguage::Markdown);

        //
        let path = Path::new("md");
        assert_eq!(MarkupLanguage::from(path), MarkupLanguage::None);
        //
        let ext = "/dir/file.md";
        assert_eq!(MarkupLanguage::from(ext), MarkupLanguage::None);

        //
        let ext = "md";
        assert_eq!(MarkupLanguage::from(ext), MarkupLanguage::Markdown);
    }
}
// `rewrite_rel_links=true`
