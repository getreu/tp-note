//! Helper functions dealing with markup languages.
use crate::config::LIB_CFG;
use std::path::Path;

/// The Markup language of the note content.
#[derive(Eq, PartialEq, Debug, Clone)]
pub enum MarkupLanguage {
    Markdown,
    RestructuredText,
    Html,
    Txt,
    Unknown,
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
        let lib_cfg = LIB_CFG.read().unwrap();

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
        if file_extension == lib_cfg.filename.extension_default {
            return MarkupLanguage::Txt;
        }
        MarkupLanguage::None
    }
}
