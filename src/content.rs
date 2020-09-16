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
    /// Constructor that reads a structured document with a YAML header
    /// and body.
    /// First `"---"` is required to start at the beginning of the `input`.
    /// Any BOM (byte order mark) at the beginning is ignored.
    /// On Windows machines it converts all `\r\n` to `\n`.
    pub fn new(input: String) -> Self {
        let input = Self::remove_bom_remove_cr(input);
        Self::split(input, false)
    }

    /// Constructor that reads a structured document with a YAML header
    /// and body.
    /// First `"---"` does not need to be at the beginning of the document.
    /// In this case all content before that place is ignored.
    /// Any BOM (byte order mark) at the beginning is ignored.
    /// On Windows machines it converts all `\r\n` to `\n`.
    pub fn new_relax(input: String) -> Self {
        let input = Self::remove_bom_remove_cr(input);
        Self::split(input, true)
    }

    #[inline]
    /// On Windows machines it converts all `\r\n` to `\n`.
    /// Any BOM (byte order mark) at the beginning is ignored.
    fn remove_bom_remove_cr(input: String) -> String {
        // Avoid allocating when there is nothing to do.
        if input.is_empty() {
            // Forward empty string.
            input
        } else if input.chars().next().unwrap_or_default() != '\u{feff}'
            && input.find('\r').is_none()
        {
            // Forward without allocating.
            input
        } else {
            // We allocate here and do a lot copying.
            input.trim_matches('\u{feff}').replace("\r\n", "\n")
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

    /// Helper function that splits the content into header and body.
    /// The header, if present, is trimmed (`trim()`), the body
    /// is kept as it is.
    ///
    /// To accept a "header" (`relax==false`):
    /// 1. the document must start with `"---"`,
    /// 2. followed by header bytes,
    /// 3. optionally followed by `"\n",
    /// 4. followed by `"---"` or `"..."`,
    /// 5. optionall followed by `"\n"`.
    /// The remaining bytes are "content".
    ///
    /// To accept a "header" (`relax==true`):
    /// A YAML metadata block may occur anywhere in the document, but
    /// if it is not at the beginning, it must be preceded by a blank line:
    /// 1. skip all until you find `"\n\n---"`
    /// 2. followed by header bytes,
    /// 3. same as above ...
    fn split(content: String, relax: bool) -> Content {
        if content.is_empty() {
            return Content::Empty;
        };

        let pattern = b"---";
        let fm_start = if content[..pattern.len()].as_bytes() == pattern {
            // Found at first byte.
            pattern.len()
        } else {
            if !relax {
                // We do not search further.
                return Content::Text(content);
            };
            let pattern = "\n\n---";
            if let Some(start) = content.find(pattern).map(|x| x + pattern.len()) {
                // Found just before `start`!
                start
            } else {
                // Not found.
                return Content::Text(content);
            }
        };

        // No need to search for an additional `\n` here, as we trim the
        // header anyway.

        let pattern1 = "\n---";
        let pattern2 = "\n...";
        let pattern_len = 4;

        let fm_end = content[fm_start..]
            .find(pattern1)
            .or_else(|| content[fm_start..].find(pattern2))
            .map(|x| x + fm_start);

        let fm_end = if let Some(n) = fm_end {
            n
        } else {
            return Content::Text(content);
        };

        // We advance 4 because `"\n---"` has 4 bytes.
        let mut body_start = fm_end + pattern_len;

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
        match &self {
            Content::Empty => write!(f, ""),
            Content::Text(t) => write!(f, "{}", t),
            Content::HeaderAndBody(h, b) => write!(f, "\u{feff}---\n{}\n---\n{}", &h, &b),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::Content;

    #[test]
    fn test_new() {
        // Test windows string.
        let content = Content::new("first\r\nsecond\r\nthird".to_string());
        assert!(matches!(content, Content::Text{..}));
        assert_eq!(content.get_body_or_text(), "first\nsecond\nthird");

        // Test Unixstring.
        let content = Content::new("first\nsecond\nthird".to_string());
        assert_eq!(content.get_body_or_text(), "first\nsecond\nthird");
        assert!(matches!(content, Content::Text{..}));

        // Test BOM removal.
        let content = Content::new("\u{feff}first\nsecond\nthird".to_string());
        assert_eq!(content.get_body_or_text(), "first\nsecond\nthird");
        assert!(matches!(content, Content::Text{..}));

        // Test header extraction.
        let content = Content::new("\u{feff}---\nfirst\n---\nsecond\nthird".to_string());
        assert_eq!(content.get_header(), "first");
        assert_eq!(content.get_body_or_text(), "second\nthird");
        assert!(matches!(content, Content::HeaderAndBody{..}));

        // Test header extraction without `\n` at the end.
        let content = Content::new("\u{feff}---\nfirst\n---".to_string());
        assert_eq!(content.get_header(), "first");
        assert_eq!(content.get_body_or_text(), "");
        assert!(matches!(content, Content::HeaderAndBody{..}));

        // This fails to find the header.
        let content = Content::new("\u{feff}not ignored\n\n---\nfirst\n---".to_string());
        assert_eq!(content.get_header(), "");
        assert_eq!(content.get_body_or_text(), "not ignored\n\n---\nfirst\n---");
        assert!(matches!(content, Content::Text{..}));
    }

    #[test]
    fn test_new_relax() {
        // The same a in the example above.
        let content = Content::new_relax("\u{feff}---\nfirst\n---".to_string());
        assert_eq!(content.get_header(), "first");
        assert_eq!(content.get_body_or_text(), "");
        assert!(matches!(content, Content::HeaderAndBody{..}));

        // Here you can see the effect of relax.
        let content = Content::new_relax("\u{feff}ignored\n\n---\nfirst\n---".to_string());
        assert_eq!(content.get_header(), "first");
        assert_eq!(content.get_body_or_text(), "");
        assert!(matches!(content, Content::HeaderAndBody{..}));
    }

    #[test]
    fn test_to_osstring() {
        let content = Content::new("---first\r\n---\r\nsecond\r\nthird".to_string());
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
        let result = Content::split(input_stream, false);
        assert_eq!(result, expected);

        let input_stream = String::from("---\nfirst\n---\nsecond\nthird");
        let expected = Content::HeaderAndBody("first".to_string(), "second\nthird".to_string());
        let result = Content::split(input_stream, false);
        assert_eq!(result, expected);

        // Header is trimmed.
        let input_stream = String::from("---\n\nfirst\n\n---\nsecond\nthird");
        let expected = Content::HeaderAndBody("first".to_string(), "second\nthird".to_string());
        let result = Content::split(input_stream, false);
        assert_eq!(result, expected);

        // Body is kept as it is (not trimmed).
        let input_stream = String::from("---\nfirst\n---\n\nsecond\nthird\n");
        let expected = Content::HeaderAndBody("first".to_string(), "\nsecond\nthird\n".to_string());
        let result = Content::split(input_stream, false);
        assert_eq!(result, expected);

        let input_stream = String::from("\nsecond\nthird");
        let expected = Content::Text("\nsecond\nthird".to_string());
        let result = Content::split(input_stream, false);
        assert_eq!(result, expected);

        let input_stream = String::from("");
        let expected = Content::Empty;
        let result = Content::split(input_stream, false);
        assert_eq!(result, expected);
    }

    #[test]
    fn test_display_for_content() {
        let input = Content::HeaderAndBody("first".to_string(), "\nsecond\nthird\n".to_string());
        let expected = "\u{feff}---\nfirst\n---\n\nsecond\nthird\n".to_string();
        assert_eq!(input.to_string(), expected);

        let input = Content::Text("\nsecond\nthird".to_string());
        let expected = "\nsecond\nthird".to_string();
        assert_eq!(input.to_string(), expected);

        let input = Content::Empty;
        let expected = "".to_string();
        assert_eq!(input.to_string(), expected);
    }
}
