//! Deals with the note's content string.

use std::fmt;

#[derive(Debug, PartialEq)]
/// This is a newtype and thin wrapper around the note's content.
/// It deals with operating system specific handling of newlines.
pub struct Content {
    /// The YAML header without `---`.
    pub header: String,
    /// Everything after the YAML header.
    pub body: String,
}

/// The content of a note is stored in some Rust-like utf-8 string with
/// one `\n` character as newline. If present, a Byte Order Mark
/// BOM is removed while reading with `new()`.
impl Content {
    /// Reads also notes created on Windows machines: in this
    /// case it converts all `\r\n` to `\n`.
    pub fn new(input: &str) -> Self {
        let content = input.trim_matches('\u{feff}').replace("\r\n", "\n");
        let (header, body) = Self::split(&content);

        Self {
            header: header.to_string(),
            body: body.to_string(),
        }
    }

    /// Write out the content string to be saved on disk.
    /// The format varies depending on the operating system:
    /// On Unix a newline is represented by one single byte: `\n`.
    /// On Windows a newline consists of two bytes: `\r\n`.
    #[allow(clippy::let_and_return)]
    pub fn to_osstring(&self) -> String {
        let s = self.to_string();

        // Replaces Windows newline + carriage return -> newline.
        #[cfg(target_family = "windows")]
        let s = (&s).replace("\n", "\r\n");
        // Under Unix no conversion is needed.
        s
    }

    /// Helper function that splits the content into header and body
    pub fn split(content: &str) -> (&str, &str) {
        let fm_start = content.find("---").map(|x| x + 3);
        if fm_start.is_none() {
            return ("", content);
        };
        let fm_start = fm_start.unwrap();

        let fm_end = content[fm_start..]
            .find("---\n")
            .or_else(|| content[fm_start..].find("...\n"))
            .map(|x| x + fm_start);

        if fm_end.is_none() {
            return ("", content);
        };
        let fm_end = fm_end.unwrap();

        let body_start = fm_end + 4;

        (&content[fm_start..fm_end].trim(), &content[body_start..])
    }
}

/// Concatenates the header and the body and prints the content.
impl fmt::Display for Content {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "\u{feff}---\n{}\n---\n{}", &self.header, &self.body)
    }
}

#[cfg(test)]
mod tests {
    use super::Content;

    #[test]
    fn test_new() {
        // test windows string
        let content = Content::new("first\r\nsecond\r\nthird");
        assert_eq!(content.body, "first\nsecond\nthird");
        // test Unixstring
        let content = Content::new("first\nsecond\nthird");
        assert_eq!(content.body, "first\nsecond\nthird");
        // test BOM removal
        let content = Content::new("\u{feff}first\nsecond\nthird");
        assert_eq!(content.body, "first\nsecond\nthird");
        // test header extraction
        let content = Content::new("\u{feff}---\nfirst\n---\nsecond\nthird");
        assert_eq!(content.header, "first");
        assert_eq!(content.body, "second\nthird");
    }

    #[test]
    fn test_to_osstring() {
        let content = Content::new("---first\r\n---\r\nsecond\r\nthird");
        let s = content.to_osstring();
        #[cfg(target_family = "windows")]
        assert_eq!(s.as_str(), "\u{feff}---\r\nfirst\r\n---\r\nsecond\r\nthird");
        #[cfg(not(target_family = "windows"))]
        assert_eq!(s.as_str(), "\u{feff}---\nfirst\n---\nsecond\nthird");
    }
}
