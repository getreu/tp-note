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
    /// We can not determine the markup language, but confirm that this
    /// is a Tp-Note file.
    Unknown,
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

    /// Returns the MIME type for all Markup Languages, Tp-Note is
    /// able to render. Otherwise, for `MarkupLanguage::Unknown` and
    /// `MarkupLanguage::None` this returns the empty string "".
    pub fn mine_type(&self) -> &'static str {
        match self {
            Self::Markdown => "text/markodwn",
            Self::Restructuredtext => "x-rst",
            Self::Html => "text/html",
            Self::PlainText => "text/plain",
            _ => "",
        }
    }

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

            Self::PlainText => text_links2html(input),

            _ => text_rawlinks2html(input),
        }
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

        for e in &lib_cfg.filename.extensions {
            if e.0 == file_extension {
                return e.1;
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
            return MarkupLanguage::PlainText;
        }

        // Nothing was found.
        MarkupLanguage::None
    }
}
