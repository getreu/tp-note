//! Deals with the note's content string.
use std::ops::Deref;

#[derive(Debug, PartialEq)]
/// This is a newtype and thin wrapper around the note's content.
/// It deals with operating system specific handling of newlines.
pub struct Content {
    s: String,
}

/// The content of a note is stored in some Rust-like utf-8 string with
/// one `\n` character as newline. If present, a Byte Order Mark
/// BOM is removed while reading with `new()`.
impl Content {
    /// Reads also notes created on Windows machines: in this
    /// case it converts all /// `\r\n` to `\n`.
    pub fn new(input: &str) -> Self {
        Self {
            s: input.trim_matches('\u{feff}').replace("\r\n", "\n"),
        }
    }

    /// Write out the content string to be saved on disk.
    /// The format varies depending on the operating system:
    /// On Unix a newline is represented by one single byte: `\n`.
    /// On Windows a newline consists of two bytes: `\r\n`.
    pub fn to_osstring(&self) -> String {
        // Replaces Windows newline + carriage return -> newline.
        #[cfg(target_family = "windows")]
        let mut s = self.replace("\n", "\r\n");
        #[cfg(target_family = "windows")]
        s.insert(0, '\u{feff}');
        
        // Under Unix no conversion is needed.
        #[cfg(not(target_family = "windows"))]
        let mut s = "\u{feff}".to_string(); 
        #[cfg(not(target_family = "windows"))]
        s.push_str(self);
        s
    }
}

/// Automatically dereference the newtype's inner string.
impl Deref for Content {
    type Target = String;

    fn deref(&self) -> &Self::Target {
        &self.s
    }
}

#[cfg(test)]
mod tests {
    use super::Content;

    #[test]
    fn test_new() {
        // test windows string
        let content = Content::new("first\r\nsecond\r\nthird");
        assert_eq!(content.as_str(), "first\nsecond\nthird");
        // test Unixstring
        let content = Content::new("first\nsecond\nthird");
        assert_eq!(content.as_str(), "first\nsecond\nthird");
        // test BOM removal
        let content = Content::new("\u{feff}first\nsecond\nthird");
        assert_eq!(content.as_str(), "first\nsecond\nthird");
    }

    #[test]
    fn test_to_osstring() {
        let content = Content::new("first\r\nsecond\r\nthird");
        let s = content.to_osstring();
        #[cfg(target_family = "windows")]
        assert_eq!(s.as_str(), "\u{feff}first\r\nsecond\r\nthird");
        #[cfg(not(target_family = "windows"))]
        assert_eq!(s.as_str(), "\u{feff}first\nsecond\nthird");
    }
}
