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

pub(crate) struct TpClipboard;

impl TpClipboard {
    /// Get a snapshot of the Markdown representation of the clipboard content.
    #[cfg(feature = "read-clipboard")]
    pub(crate) fn get_content() -> Option<String> {
        #[cfg(unix)]
        let wl_clipboard = match paste::get_contents(
            paste::ClipboardType::Regular,
            paste::Seat::Unspecified,
            paste::MimeType::TextWithPriority("text/html"),
        ) {
            Ok((mut pipe_reader, _mime_type)) => {
                let mut buffer = String::new();
                match pipe_reader.read_to_string(&mut buffer) {
                    Ok(l) if l > 0 => {
                        //if mime_type == "text/html" ..
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

            let mut buffer = ctx.get_html().or_else(|_| ctx.get_text()).ok()?;

            // `trim_end()` content without new allocation.
            buffer.truncate(buffer.trim_end().len());
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
