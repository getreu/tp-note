//! Abstact the clipboard handling.

#[cfg(feature = "read-clipboard")]
use clipboard_rs::Clipboard;
#[cfg(feature = "read-clipboard")]
use clipboard_rs::ClipboardContext;
#[cfg(feature = "read-clipboard")]
#[cfg(unix)]
use std::io::Read as _;
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

pub(crate) struct TpClipboard;

impl TpClipboard {
    /// Get a snapshot of the Markdown representation of the clipboard content.
    /// If the content contains HTML, the marker `<!DOCTYPE html>` is
    /// prepended.
    #[cfg(feature = "read-clipboard")]
    pub(crate) fn get_content() -> Option<String> {
        // Query Wayland clipboard.
        #[cfg(unix)]
        let wl_clipboard = match paste::get_contents(
            paste::ClipboardType::Regular,
            paste::Seat::Unspecified,
            #[cfg(feature = "html-clipboard")]
            paste::MimeType::TextWithPriority("text/html"),
            #[cfg(not(feature = "html-clipboard"))]
            paste::MimeType::Text,
        ) {
            Ok((mut pipe_reader, mime_type)) => {
                let mut buffer = String::new();
                match pipe_reader.read_to_string(&mut buffer) {
                    Ok(l) if l > 0 => {
                        if mime_type == "text/html"
                            && !buffer.trim_start().is_empty()
                            && !buffer
                                .lines()
                                .next()
                                .map(|l| l.trim_start().to_ascii_lowercase())
                                .is_some_and(|l| l.starts_with(HTML_PAT1))
                        {
                            buffer.insert_str(0, HTML_PAT2);
                        }
                        Some(buffer)
                    }
                    _ => None,
                }
            }
            _ => None,
        };

        #[cfg(not(unix))]
        let wl_clipboard = None;

        wl_clipboard.or_else(|| {
            // Query X11 keyboard.
            let ctx: ClipboardContext = ClipboardContext::new().ok()?;
            #[cfg(feature = "html-clipboard")]
            let buffer = if let Ok(mut html) = ctx.get_html() {
                if !html.trim_start().is_empty()
                    && !html
                        .lines()
                        .next()
                        .map(|l| l.trim_start().to_ascii_lowercase())
                        .is_some_and(|l| l.starts_with(HTML_PAT1))
                {
                    html.insert_str(0, HTML_PAT2);
                }
                html
            } else {
                ctx.get_text().unwrap_or_default()
            };

            #[cfg(not(feature = "html-clipboard"))]
            let buffer = ctx.get_text().unwrap_or_default();

            Some(buffer)
        })
    }

    /// When the `read-clipboard` feature is disabled, always return `None`.
    #[inline]
    #[cfg(not(feature = "read-clipboard"))]
    pub(crate) fn get_content() -> Option<String> {
        None
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
