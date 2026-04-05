//! Abstract the clipboard handling.

#[cfg(feature = "read-clipboard")]
use clipboard_rs::Clipboard;
#[cfg(feature = "read-clipboard")]
use clipboard_rs::ClipboardContext;
use tpnote_lib::config::TMPL_VAR_HTML_CLIPBOARD;
use tpnote_lib::config::TMPL_VAR_TXT_CLIPBOARD;
use tpnote_lib::content::Content;
use tpnote_lib::content::ContentString;
#[cfg(feature = "read-clipboard")]
use tpnote_lib::text_reader::StringExt;
#[cfg(feature = "read-clipboard")]
#[derive(Debug, PartialEq, Eq)]
pub struct SystemClipboard {
    pub html: ContentString,
    pub txt: ContentString,
}

impl SystemClipboard {
    /// Get a snapshot of the Markdown representation of the clipboard content.
    /// If the content contains HTML, the marker `<!DOCTYPE html>` is
    /// prepended.
    #[cfg(feature = "read-clipboard")]
    pub(crate) fn new() -> Self {
        // Bring new methods into scope.
        use tpnote_lib::{
            config::{TMPL_VAR_HTML_CLIPBOARD, TMPL_VAR_TXT_CLIPBOARD},
            content::Content,
        };

        let mut txt_content = String::new();
        let mut html_content = String::new();

        // Query clipboard.
        if let Ok(ctx) = ClipboardContext::new() {
            if let Ok(html) = ctx.get_html() {
                // As this is HTML what the newline kind does not matter
                // here.
                log::trace!("Got HTML clipboard:\n {}", html_content);
                html_content = html;
            };
            if let Ok(txt) = ctx.get_text() {
                txt_content = txt.crlf_suppressor_string();
                log::trace!("Got text clipboard:\n {}", txt_content);
            };
        }

        Self {
            html: ContentString::from_html(html_content, TMPL_VAR_HTML_CLIPBOARD.to_string())
                .map_err(|e| {
                    log::error!("Could not interpret HTML clipboard:\n{}", e);
                })
                // Ignore error and continue with empty string.
                .unwrap_or_default(),
            txt: ContentString::from_string(txt_content, TMPL_VAR_TXT_CLIPBOARD.to_string()),
        }
    }

    /// When the `read-clipboard` feature is disabled, always return `None`.
    #[inline]
    #[cfg(not(feature = "read-clipboard"))]
    pub(crate) fn new() -> Self {
        Self::default()
    }

    /// Empty the clipboard.
    #[inline]
    pub(crate) fn empty() {
        // Clear X11 and other clipboards.
        #[cfg(feature = "read-clipboard")]
        if let Ok(ctx) = clipboard_rs::ClipboardContext::new() {
            let _ = ctx.set_html("".to_string());
            let _ = ctx.set_text("".to_string());
        };
    }
}

/// Register empty HTML and TXT clipboards with the `ContentString` names:
/// * `TMPL_VAR_HTML_CLIPBOARD`,
/// * `TMPL_VAR_TXT_CLIPBOARD`.
impl Default for SystemClipboard {
    fn default() -> Self {
        SystemClipboard {
            html: ContentString::from_string(String::new(), TMPL_VAR_HTML_CLIPBOARD.to_string()),
            txt: ContentString::from_string(String::new(), TMPL_VAR_TXT_CLIPBOARD.to_string()),
        }
    }
}
