//! Abstact the clipboard handling.

#[cfg(feature = "read-clipboard")]
use clipboard_rs::Clipboard;
#[cfg(feature = "read-clipboard")]
use clipboard_rs::ClipboardContext;
#[cfg(feature = "read-clipboard")]
#[cfg(unix)]
use std::io::Read as _;
use tpnote_lib::content::ContentString;
#[cfg(feature = "read-clipboard")]
#[cfg(unix)]
use wl_clipboard_rs::copy;
#[cfg(feature = "read-clipboard")]
#[cfg(unix)]
use wl_clipboard_rs::paste;

/// Lowercase test pattern to check if there is a document type declaration
/// already.
#[cfg(feature = "read-clipboard")]
const HTML_PAT1: &str = "<!doctype ";
/// Prepend a marker declaration, in case the content is HTML.
#[cfg(feature = "read-clipboard")]
const HTML_PAT2: &str = "<!DOCTYPE html>";

#[derive(Default, Debug, PartialEq, Eq)]
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
        use tpnote_lib::content::Content;

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
                        if !html_content.trim_start().is_empty()
                            && !html_content
                                .lines()
                                .next()
                                .map(|l| l.trim_start().to_ascii_lowercase())
                                .is_some_and(|l| l.starts_with(HTML_PAT1))
                        {
                            html_content.insert_str(0, HTML_PAT2);
                        }
                    }
                    _ => {}
                }
            };
            // Plain teext clipboard content
            if let Ok((mut pipe_reader, _)) = paste::get_contents(
                paste::ClipboardType::Regular,
                paste::Seat::Unspecified,
                paste::MimeType::Specific("plain/text"),
            ) {
                let _ = pipe_reader.read_to_string(&mut txt_content);
            };
        }

        if html_content.is_empty() && txt_content.is_empty() {
            // Query X11 clipboard.
            if let Ok(ctx) = ClipboardContext::new() {
                if let Ok(mut html) = ctx.get_html() {
                    if !html.trim_start().is_empty()
                        && !html
                            .lines()
                            .next()
                            .map(|l| l.trim_start().to_ascii_lowercase())
                            .is_some_and(|l| l.starts_with(HTML_PAT1))
                    {
                        html.insert_str(0, HTML_PAT2);
                    }
                    html_content = html;
                };
                if let Ok(txt) = ctx.get_text() {
                    txt_content = txt;
                };
            }
        }

        Self {
            html: ContentString::from_string_with_cr(html_content),
            txt: ContentString::from_string_with_cr(txt_content),
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
