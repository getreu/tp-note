//! Helper functions dealing with markup languages.
use crate::config::LIB_CFG;
use crate::error::NoteError;
#[cfg(feature = "renderer")]
use crate::highlight::SyntaxPreprocessor;
#[cfg(feature = "renderer")]
use crate::html2md::convert_html_to_md;
use crate::settings::SETTINGS;
use parse_hyperlinks::renderer::text_links2html;
use parse_hyperlinks::renderer::text_rawlinks2html;
#[cfg(feature = "renderer")]
use pulldown_cmark::{html, Options, Parser};
#[cfg(feature = "renderer")]
use rst_parser;
#[cfg(feature = "renderer")]
use rst_renderer;
use serde::{Deserialize, Serialize};
use std::path::Path;
#[cfg(feature = "renderer")]
use std::str::from_utf8;

/// The filter `filter_tags()` ommits HTML `<span....>` after converting to
/// Markdown.
#[cfg(test)] // Currently the `filter_tags()` filter is not used in the code.
#[cfg(feature = "renderer")]
const FILTERED_TAGS: &[&str; 4] = &["<span", "</span>", "<div", "</div>"];

/// Availble converters for converting the input from stdin or the clipboard
/// to HTML.
#[non_exhaustive]
#[derive(Default, Debug, Hash, Clone, Eq, PartialEq, Deserialize, Serialize, Copy)]
pub enum InputConverter {
    /// Convert from HTML to Markdown.
    ToMarkdown,
    /// Do not convert, return an error instead.
    #[default]
    Disabled,
    /// Do not convert, just pass through wrapped in `Ok()`.
    PassThrough,
}

impl InputConverter {
    /// Returns a function that implements the `InputConverter` looked up in
    /// the `extensions` table in the `extension` line.
    /// When `extension` is not found in `extensions`, the function returns
    /// an NoteError.
    #[inline]
    pub(crate) fn build(extension: &str) -> fn(String) -> Result<String, NoteError> {
        let settings = SETTINGS.read_recursive();
        let scheme = &LIB_CFG.read_recursive().scheme[settings.current_scheme];

        let mut input_converter = InputConverter::default();
        for e in &scheme.filename.extensions {
            if e.0 == *extension {
                input_converter = e.1;
                break;
            }
        }

        match input_converter {
            #[cfg(feature = "renderer")]
            InputConverter::ToMarkdown => |s| convert_html_to_md(&s),

            InputConverter::Disabled => {
                |_: String| -> Result<String, NoteError> { Err(NoteError::HtmlToMarkupDisabled) }
            }

            _ => Ok,
        }
    }

    /// Filters the `TARGET_TAGS`, e.g. `<span...>`, `</span>`, `<div...>`
    /// and `<div>` in `text`.
    /// Contract: the input substring `...` does not contain the characters
    /// `>` or `\n`.
    #[cfg(test)] // Currently the `filter_tags()` filter is not used in the code.
    #[cfg(feature = "renderer")]
    fn filter_tags(text: String) -> String {
        let mut res = String::new();
        let mut i = 0;
        while let Some(mut start) = text[i..].find('<') {
            if let Some(mut end) = text[i + start..].find('>') {
                end += 1;
                // Move on if there is another opening bracket.
                if let Some(new_start) = text[i + start + 1..i + start + end].rfind('<') {
                    start += new_start + 1;
                    end -= new_start + 1;
                }

                // Is this a tag listed in `FILTERED_TAGS`?
                let filter_tag = FILTERED_TAGS
                    .iter()
                    .any(|&pat| text[i + start..i + start + end].starts_with(pat));

                if filter_tag {
                    res.push_str(&text[i..i + start]);
                } else {
                    res.push_str(&text[i..i + start + end]);
                };
                i = i + start + end;
            } else {
                res.push_str(&text[i..i + start + 1]);
                i = i + start + 1;
            }
        }
        if i > 0 {
            res.push_str(&text[i..]);
            if res != text {
                log::trace!("`html_to_markup` filter: removed tags in \"{}\"", text);
            }
            res
        } else {
            text
        }
    }
}

/// The Markup language of the note content.
#[non_exhaustive]
#[derive(Default, Debug, Hash, Clone, Eq, PartialEq, Deserialize, Serialize, Copy)]
pub enum MarkupLanguage {
    Markdown,
    ReStructuredText,
    Html,
    PlainText,
    /// The markup langugae is known, but the renderer is disabled.
    RendererDisabled,
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
            Self::ReStructuredText => Some("x-rst"),
            Self::Html => Some("text/html"),
            Self::PlainText => Some("text/plain"),
            Self::RendererDisabled => Some("text/plain"),
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
    ///   `ReStructuredText`. This feature is experimental.
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
            Self::ReStructuredText => {
                // Note, that the current rst renderer requires files to end with a new line.
                // <https://github.com/flying-sheep/rust-rst/issues/30>
                let rest_input = input.trim_start();
                // Write to String buffer.
                let mut html_output: Vec<u8> = Vec::with_capacity(rest_input.len() * 3 / 2);
                const STANDALONE: bool = false; // Don't wrap in `<!doctype html><html></html>`.
                rst_parser::parse(rest_input.trim_start())
                    .map(|doc| rst_renderer::render_html(&doc, &mut html_output, STANDALONE))
                    .map_or_else(
                        |e| NoteError::RstParse { msg: e.to_string() }.to_string(),
                        |_| from_utf8(&html_output).unwrap_or_default().to_string(),
                    )
            }

            Self::Html => input.to_string(),

            Self::PlainText | Self::RendererDisabled => text_links2html(input),

            Self::Unkown => text_rawlinks2html(input),

            _ => String::new(),
        }
    }
}

impl From<&Path> for MarkupLanguage {
    /// Is the file extension ` at the end of the given path listed in
    /// `file.extensions`?  Return the corresponding `MarkupLanguage`.
    /// Only the extension of `Path` is condidered here.
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
        let scheme = &LIB_CFG.read_recursive().scheme[SETTINGS.read_recursive().current_scheme];

        for e in &scheme.filename.extensions {
            if e.0 == file_extension {
                return e.2;
            }
        }

        // Nothing was found.
        MarkupLanguage::None
    }
}

#[cfg(test)]
mod tests {

    use super::InputConverter;
    use super::MarkupLanguage;
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

        //
        let ext = "rst";
        assert_eq!(MarkupLanguage::from(ext), MarkupLanguage::ReStructuredText);
    }

    #[test]
    fn test_markuplanguage_render() {
        // Markdown
        let input = "[Link text](https://domain.invalid/)";
        let expected: &str = "<p><a href=\"https://domain.invalid/\">Link text</a></p>\n";

        let result = MarkupLanguage::Markdown.render(input);
        assert_eq!(result, expected);

        // ReStructuredText
        let input = "`Link text <https://domain.invalid/>`_";
        let expected: &str = "<p><a href=\"https://domain.invalid/\">Link text</a></p>\n";

        let result = MarkupLanguage::ReStructuredText.render(input);
        assert_eq!(result, expected);
    }

    #[test]
    fn test_input_converter_md() {
        let ic = InputConverter::build("md");
        let input: &str =
            "<div id=\"videopodcast\">outside <span id=\"pills\">inside</span>\n</div>";
        let expected: &str = "outside inside";

        let result = ic(input.to_string());
        assert_eq!(result.unwrap(), expected);

        //
        // [Commonmark: Example 489](https://spec.commonmark.org/0.31.2/#example-489)
        let input: &str = r#"<p><a href="/my uri">link</a></p>"#;
        let expected: &str = "[link](</my uri>)";

        let result = ic(input.to_string());
        assert_eq!(result.unwrap(), expected);

        //
        // [Commonmark: Example 489](https://spec.commonmark.org/0.31.2/#example-489)
        let input: &str = r#"<p><a href="/my%20uri">link</a></p>"#;
        let expected: &str = "[link](</my uri>)";

        let result = ic(input.to_string());
        assert_eq!(result.unwrap(), expected);

        //
        // We want ATX style headers.
        let input: &str = r#"<p><h1>Title</h1></p>"#;
        let expected: &str = "# Title";

        let result = ic(input.to_string());
        assert_eq!(result.unwrap(), expected);
    }

    #[test]
    fn test_filter_tags() {
        let input: &str =
            "A<div id=\"videopodcast\">out<p>side <span id=\"pills\">inside</span>\n</div>B";
        let expected: &str = "Aout<p>side inside\nB";

        let result = InputConverter::filter_tags(input.to_string());
        assert_eq!(result, expected);

        let input: &str = "A<B<C <div>D<E<p>F<>G";
        let expected: &str = "A<B<C D<E<p>F<>G";

        let result = InputConverter::filter_tags(input.to_string());
        assert_eq!(result, expected);
    }
}
// `rewrite_rel_links=true`
