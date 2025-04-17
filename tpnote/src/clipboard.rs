//! Abstract the clipboard handling.

#[cfg(feature = "read-clipboard")]
use clipboard_rs::Clipboard;
#[cfg(feature = "read-clipboard")]
use clipboard_rs::ClipboardContext;
#[cfg(feature = "read-clipboard")]
#[cfg(unix)]
use std::io::Read as _;
use tpnote_lib::config::TMPL_VAR_HTML_CLIPBOARD;
use tpnote_lib::config::TMPL_VAR_HTML_CLIPBOARD_HEADER;
use tpnote_lib::config::TMPL_VAR_TXT_CLIPBOARD;
use tpnote_lib::config::TMPL_VAR_TXT_CLIPBOARD_HEADER;
use tpnote_lib::content::Content;
use tpnote_lib::content::ContentString;
#[cfg(feature = "read-clipboard")]
#[cfg(unix)]
use wl_clipboard_rs::copy;
#[cfg(feature = "read-clipboard")]
#[cfg(unix)]
use wl_clipboard_rs::paste;

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
            config::{
                TMPL_VAR_HTML_CLIPBOARD, TMPL_VAR_HTML_CLIPBOARD_HEADER, TMPL_VAR_TXT_CLIPBOARD,
                TMPL_VAR_TXT_CLIPBOARD_HEADER,
            },
            content::Content,
        };

        let mut txt_content = String::new();
        let mut html_content = String::new();

        #[cfg(unix)]
        {
            // Query Wayland clipboard
            // Html clipboard content
            if let Ok((mut pipe_reader, _)) = paste::get_contents(
                paste::ClipboardType::Regular,
                paste::Seat::Unspecified,
                paste::MimeType::Specific("text/html"),
            ) {
                match pipe_reader.read_to_string(&mut html_content) {
                    Ok(l) if l > 0 => {
                        log::trace!("Got HTML Wayland clipboard:\n {}", html_content);
                    }
                    _ => {}
                }
            };
            // Plain text clipboard content
            if let Ok((mut pipe_reader, _)) = paste::get_contents(
                paste::ClipboardType::Regular,
                paste::Seat::Unspecified,
                paste::MimeType::Specific("plain/text"),
            ) {
                let _ = pipe_reader.read_to_string(&mut txt_content);
                log::trace!("Got text Wayland clipboard:\n {}", txt_content);
            };
        }

        if html_content.is_empty() && txt_content.is_empty() {
            // Query X11 clipboard.
            if let Ok(ctx) = ClipboardContext::new() {
                if let Ok(html) = ctx.get_html() {
                    log::trace!("Got HTML non-wayland clipboard:\n {}", html_content);
                    html_content = html;
                };
                if let Ok(txt) = ctx.get_text() {
                    txt_content = txt;
                    log::trace!("Got text non-wayland clipboard:\n {}", txt_content);
                };
            }
        }

        Self {
            html: ContentString::from_html(
                html_content,
                TMPL_VAR_HTML_CLIPBOARD_HEADER.to_string(),
                TMPL_VAR_HTML_CLIPBOARD.to_string(),
            )
            .map_err(|e| {
                log::error!("Could not interpret HTML clipboard:\n{}", e);
            })
            // Ignore error and continue with empty string.
            .unwrap_or_default(),
            txt: ContentString::from_string(
                txt_content,
                TMPL_VAR_TXT_CLIPBOARD_HEADER.to_string(),
                TMPL_VAR_TXT_CLIPBOARD.to_string(),
            ),
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
        // Clear Wayland clipboard.
        #[cfg(feature = "read-clipboard")]
        #[cfg(unix)]
        let _ = copy::clear(copy::ClipboardType::Regular, copy::Seat::All);

        // Clear X11 and other clipboards.
        #[cfg(feature = "read-clipboard")]
        if let Ok(ctx) = clipboard_rs::ClipboardContext::new() {
            let _ = ctx.set_html("".to_string());
            let _ = ctx.set_text("".to_string());
        };
    }
}

/// Register empty HTML and TXT clipboards with the `ContentString` names:
/// * `TMPL_VAR_HTML_CLIPBOARD_HEADER`,
/// * `TMPL_VAR_HTML_CLIPBOARD`,
/// * `TMPL_VAR_TXT_CLIPBOARD_HEADER` and
/// * `TMPL_VAR_TXT_CLIPBOARD`.
impl Default for SystemClipboard {
    fn default() -> Self {
        SystemClipboard {
            html: ContentString::from_string(
                String::new(),
                TMPL_VAR_HTML_CLIPBOARD_HEADER.to_string(),
                TMPL_VAR_HTML_CLIPBOARD.to_string(),
            ),
            txt: ContentString::from_string(
                String::new(),
                TMPL_VAR_TXT_CLIPBOARD_HEADER.to_string(),
                TMPL_VAR_TXT_CLIPBOARD.to_string(),
            ),
        }
    }
}
