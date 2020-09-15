//! Deals with the note's content string.

use std::fmt;

#[derive(Debug, PartialEq)]
/// This is a newtype and thin wrapper around the note's content.
/// It deals with operating system specific handling of newlines.
pub enum Content {
    /// The first string is the trimmed YAML header without `---`, the second
    /// the body as it is (without `trim()`).
    /// All line endings are converted to UNIX line endings `\n`.
    HeaderAndBody(String, String),
    /// The raw unstructured input.
    /// All line endings are converted to UNIX line endings `\n`.
    Text(String),
    Empty,
}

/// The content of a note is stored in some Rust-like utf-8 string with
/// one `\n` character as newline. If present, a Byte Order Mark
/// BOM is removed while reading with `new()`.
impl Content {
    /// Reads also notes created on Windows machines: in this
    /// case it converts all `\r\n` to `\n`.
    pub fn new(input: &str) -> Self {
        let content = input.trim_matches('\u{feff}').replace("\r\n", "\n");
        Self::split(content)
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

    /// Helper function that splits the content into header and body.
    /// The header, if present, is trimmed (`trim()`), the body
    /// is kept as it is.
    /// To accept a "header", the document must:
    /// 1. start with `"---"`,
    /// 2. followed by header bytes,
    /// 3.  optionally followed by `"\n",
    /// 4. followed by `"---"` or `"..."`,
    /// 5. optionall followed by `"\n"`.
    /// The remaining bytes are "content".
    pub fn split(content: String) -> Content {
        if content.is_empty() {
            return Content::Empty;
        };

        let fm_start = if content[..4].as_bytes() == b"---\n" {
            // Should be evaluated at compile time to 4.
            b"---\n".len()
        } else if content[..3].as_bytes() == b"---" {
            b"---".len()
        } else {
            return Content::Text(content);
        };

        let fm_end = content[fm_start..]
            .find("\n---")
            .or_else(|| content[fm_start..].find("\n..."))
            .map(|x| x + fm_start);

        let fm_end = if let Some(n) = fm_end {
            n
        } else {
            return Content::Text(content);
        };

        // We advance 4 because `"\n---"` has 4 bytes.
        let mut body_start = fm_end + 4;

        // Skip potential newline.
        if (content.len() > body_start) && (content.as_bytes()[body_start] == b'\n') {
            body_start += 1;
        };

        Content::HeaderAndBody(
            content[fm_start..fm_end].trim().to_string(),
            content[body_start..].to_string(),
        )
    }

    /// Getter for header. If it does not exist in a variant return `""`.
    pub fn get_header(&self) -> &str {
        match &self {
            Content::Empty => "",
            Content::Text(_) => "",
            Content::HeaderAndBody(h, _) => h,
        }
    }

    /// Getter for body. If it does not exist in a variant return `""`.
    pub fn get_body_or_text(&self) -> &str {
        match &self {
            Content::Empty => "",
            Content::Text(b) => b,
            Content::HeaderAndBody(_, b) => b,
        }
    }
}

/// Concatenates the header and the body and prints the content.
impl fmt::Display for Content {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "\u{feff}---\n{}\n---\n{}",
            &self.get_header(),
            &self.get_body_or_text()
        )
    }
}

#[cfg(test)]
mod tests {
    use super::Content;

    #[test]
    fn test_new() {
        // Test windows string.
        let content = Content::new("first\r\nsecond\r\nthird");
        assert_eq!(content.get_body_or_text(), "first\nsecond\nthird");
        // Test Unixstring.
        let content = Content::new("first\nsecond\nthird");
        assert_eq!(content.get_body_or_text(), "first\nsecond\nthird");
        // Test BOM removal.
        let content = Content::new("\u{feff}first\nsecond\nthird");
        assert_eq!(content.get_body_or_text(), "first\nsecond\nthird");
        // Test header extraction.
        let content = Content::new("\u{feff}---\nfirst\n---\nsecond\nthird");
        assert_eq!(content.get_header(), "first");
        assert_eq!(content.get_body_or_text(), "second\nthird");
        // Test header extraction without `\n` at the end
        let content = Content::new("\u{feff}---\nfirst\n---");
        assert_eq!(content.get_header(), "first");
        assert_eq!(content.get_body_or_text(), "");
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

    #[test]
    fn test_split() {
        let input_stream = String::from("---first\n---\nsecond\nthird");
        let expected = Content::HeaderAndBody("first".to_string(), "second\nthird".to_string());
        let result = Content::split(input_stream);
        assert_eq!(result, expected);

        let input_stream = String::from("---\nfirst\n---\nsecond\nthird");
        let expected = Content::HeaderAndBody("first".to_string(), "second\nthird".to_string());
        let result = Content::split(input_stream);
        assert_eq!(result, expected);

        // Header is trimmed.
        let input_stream = String::from("---\n\nfirst\n\n---\nsecond\nthird");
        let expected = Content::HeaderAndBody("first".to_string(), "second\nthird".to_string());
        let result = Content::split(input_stream);
        assert_eq!(result, expected);

        // Body is kept as it is (not trimmed).
        let input_stream = String::from("---\nfirst\n---\n\nsecond\nthird\n");
        let expected = Content::HeaderAndBody("first".to_string(), "\nsecond\nthird\n".to_string());
        let result = Content::split(input_stream);
        assert_eq!(result, expected);

        let input_stream = String::from("\nsecond\nthird");
        let expected = Content::Text("\nsecond\nthird".to_string());
        let result = Content::split(input_stream);
        assert_eq!(result, expected);

        let input_stream = String::from("");
        let expected = Content::Empty;
        let result = Content::split(input_stream);
        assert_eq!(result, expected);
    }
}
