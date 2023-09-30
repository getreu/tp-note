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
use std::path::Path;
#[cfg(feature = "renderer")]
use std::str::from_utf8;

/// The Markup language of the note content.
#[derive(Eq, PartialEq, Debug, Clone)]
pub enum MarkupLanguage {
    Markdown,
    RestructuredText,
    Html,
    Txt,
    /// We can not determine the markup language, but confirm that this
    /// is a Tp-Note file.
    Unknown,
    /// This is not a Tp-Note file.
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

    pub fn render(&self, input: &str) -> String {
        match self {
            #[cfg(feature = "renderer")]
            Self::Markdown => Self::render_md_content(input),
            #[cfg(feature = "renderer")]
            Self::RestructuredText => Self::render_rst_content(input),
            Self::Html => input.to_string(),
            Self::Txt => Self::render_txt_content(input),
            _ => Self::render_unknown_content(input),
        }
    }

    #[inline]
    #[cfg(feature = "renderer")]
    /// Markdown renderer.
    fn render_md_content(markdown_input: &str) -> String {
        // Set up options and parser. Besides the CommonMark standard
        // we enable some useful extras.

        let options = Options::all();
        let parser = Parser::new_ext(markdown_input, options);
        let parser = SyntaxPreprocessor::new(parser);

        // Write to String buffer.
        let mut html_output: String = String::with_capacity(markdown_input.len() * 3 / 2);
        html::push_html(&mut html_output, parser);
        html_output
    }

    #[inline]
    #[cfg(feature = "renderer")]
    /// RestructuredText renderer.
    fn render_rst_content(rest_input: &str) -> String {
        // Note, that the current rst renderer requires files to end with a new line.
        // <https://github.com/flying-sheep/rust-rst/issues/30>
        let mut rest_input = rest_input.trim_start();
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

    #[inline]
    /// Renderer for markup languages other than the above.
    fn render_txt_content(other_input: &str) -> String {
        text_links2html(other_input)
    }

    #[inline]
    /// Renderer for markup languages other than the above.
    fn render_unknown_content(other_input: &str) -> String {
        text_rawlinks2html(other_input)
    }
}

impl From<&Path> for MarkupLanguage {
    /// Is `file_extension` listed in one of the known file extension
    /// lists?
    #[inline]
    fn from(file_extension: &Path) -> Self {
        let file_extension = file_extension
            .extension()
            .unwrap_or_default()
            .to_str()
            .unwrap_or_default();

        Self::from(file_extension)
    }
}

impl From<&str> for MarkupLanguage {
    /// Is `file_extension` listed in one of the known file extension
    /// lists?
    #[inline]
    fn from(file_extension: &str) -> Self {
        let lib_cfg = LIB_CFG.read_recursive();

        for e in &lib_cfg.filename.extensions_md {
            if e == file_extension {
                return MarkupLanguage::Markdown;
            }
        }
        for e in &lib_cfg.filename.extensions_rst {
            if e == file_extension {
                return MarkupLanguage::RestructuredText;
            }
        }
        for e in &lib_cfg.filename.extensions_html {
            if e == file_extension {
                return MarkupLanguage::Html;
            }
        }
        for e in &lib_cfg.filename.extensions_txt {
            if e == file_extension {
                return MarkupLanguage::Txt;
            }
        }
        for e in &lib_cfg.filename.extensions_no_viewer {
            if e == file_extension {
                return MarkupLanguage::Unknown;
            }
        }

        // If ever `extension_default` got forgotten in
        // one of the above lists, make sure that Tp-Note
        // recognizes its own files. Even without Markup
        // rendition.
        let settings = SETTINGS.read_recursive();

        if file_extension == lib_cfg.filename.extension_default
            || file_extension == settings.extension_default
        {
            return MarkupLanguage::Txt;
        }
        MarkupLanguage::None
    }
}
