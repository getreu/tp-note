//! Encapsulate the clipboard handling.
use tpnote_lib::content::Content;
use tpnote_lib::content::ContentString;

pub(crate) struct Clipboard;

impl Clipboard {
    /// Get snapshot of the clipboard content.
    pub(crate) fn get() -> ContentString {
        // Concatenate clipboard content.
        #[cfg(feature = "read-clipboard")]
        {
            let mut buffer = String::new();
            if let Ok(mut ctx) = arboard::Clipboard::new() {
                if let Ok(s) = ctx.get_text() {
                    buffer = s;
                }
            };

            // `trim_end()` content without new allocation.
            buffer.truncate(buffer.trim_end().len());

            ContentString::from_string_with_cr(buffer)
        }

        #[cfg(not(feature = "read-clipboard"))]
        ContentString::from_string_with_cr(String::new())
    }

    /// Empty the clipboard.
    pub fn empty() {
        #[cfg(feature = "read-clipboard")]
        if let Ok(mut ctx) = arboard::Clipboard::new() {
            let _ = ctx.set_text("".to_string());
        };
    }
}
