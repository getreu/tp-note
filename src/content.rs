//! Deals with the note's content string.

use std::fmt;

#[derive(Debug, PartialEq)]
/// This is a newtype and thin wrapper around the note's content.
/// It deals with operating system specific handling of newlines.
pub struct Content<'a> {
    /// The raw content as String, can be empty.
    // TODO Pin here.
    pub s: String,
    /// Skip optional BOM and `"---" `in `s` until next `"---"`.
    /// When no `---` is found, this is empty.
    /// `header` is always trimmed.
    pub header: &'a str,
    /// Skip optional BOM and optional header and keep the rest.
    pub body: &'a str,
}

/// This macro is useful for zero-cost conversion from &[u8] to &str. Use
/// this with care. Make sure, that the byte-slice boundaries always fit character
/// boundaries and that the slice only contains valid UTF-8. Also, check for potential
/// race conditions yourself, because this disables borrow checking for
/// `$slice_u8`.
/// This is the immutable version.
macro_rules! unborrow {
    ($slice_u8:expr) => {{
        use std::slice;
        use std::str;
        let ptr = $slice_u8.as_ptr();
        let len = $slice_u8.len();
        &unsafe { str::from_utf8_unchecked(slice::from_raw_parts(ptr, len)) }
    }};
}

/// The content of a note is stored in some Rust-like utf-8 string with
/// one `\n` character as newline. If present, a Byte Order Mark
/// BOM is removed while reading with `new()`.
impl<'a> Content<'a> {
    /// Constructor that reads a structured document with a YAML header
    /// and body.
    /// First `"---"` is required to start at the beginning of the `input`.
    /// Any BOM (byte order mark) at the beginning is ignored.
    /// On Windows machines it converts all `\r\n` to `\n`.
    pub fn new(input: String) -> Self {
        let input = Self::remove_cr(input);
        let input_ref = unborrow!(&input);
        let (header, body) = Self::split(&input_ref, false);
        Content {
            s: input,
            header,
            body,
        }
    }

    /// Constructor that reads a structured document with a YAML header
    /// and body.
    /// First `"---"` does not need to be at the beginning of the document.
    /// In this case all content before that place is ignored.
    /// Any BOM (byte order mark) at the beginning is ignored.
    /// On Windows machines it converts all `\r\n` to `\n`.
    pub fn new_relax(input: String) -> Self {
        let input = Self::remove_cr(input);
        let input_ref = unborrow!(&input);
        let (header, body) = Self::split(&input_ref, true);
        Content {
            s: input,
            header,
            body,
        }
    }

    /// Helper function that splits the content into header and body.
    /// The header, if present, is trimmed (`trim()`), the body
    /// is kept as it is.
    /// Any BOM (byte order mark) at the beginning is ignored.
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
    fn split(content: &'a str, relax: bool) -> (&'a str, &'a str) {
        // Remove BOM
        let content = content.trim_start_matches('\u{feff}');

        if content.is_empty() {
            return ("", "");
        };

        let pattern = b"---";
        let fm_start = if content[..pattern.len()].as_bytes() == pattern {
            // Found at first byte.
            pattern.len()
        } else {
            if !relax {
                // We do not search further.
                return ("", content);
            };
            let pattern = "\n\n---";
            if let Some(start) = content.find(pattern).map(|x| x + pattern.len()) {
                // Found just before `start`!
                start
            } else {
                // Not found.
                return ("", content);
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
            return ("", content);
        };

        // We advance 4 because `"\n---"` has 4 bytes.
        let mut body_start = fm_end + pattern_len;

        // Skip potential newline.
        if (content.len() > body_start) && (content.as_bytes()[body_start] == b'\n') {
            body_start += 1;
        };

        (content[fm_start..fm_end].trim(), &content[body_start..])
    }

    /// On Windows machines it converts all `\r\n` to `\n`.
    #[inline]
    fn remove_cr(input: String) -> String {
        // Avoid allocating when there is nothing to do.
        if input.find('\r').is_none() {
            // Forward without allocating.
            input
        } else {
            // We allocate here and do a lot copying.
            input.replace("\r\n", "\n")
        }
    }

    /// Write out the content string to be saved on disk.
    /// The format varies depending on the operating system:
    /// On Unix a newline is represented by one single byte: `\n`.
    /// On Windows a newline consists of two bytes: `\r\n`.
    // TODO 1. avoid allocation when there is nothing to do
    // TODO 2. do not use Display as it allocates also.
    #[allow(clippy::let_and_return)]
    pub fn to_osstring(&self) -> String {
        let s = self.to_string();

        // Replaces Windows newline + carriage return -> newline.
        #[cfg(target_family = "windows")]
        let s = (&s).replace("\n", "\r\n");
        // Under Unix no conversion is needed.
        s
    }
}

/// Concatenates the header and the body and prints the content.
impl<'a> fmt::Display for Content<'a> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let s = if self.header.is_empty() {
            self.body.to_string()
        } else {
            format!("\u{feff}---\n{}\n---\n{}", &self.header, &self.body)
        };
        write!(f, "{}", s)
    }
}

#[cfg(test)]
mod tests {
    use super::Content;

    #[test]
    fn test_new() {
        // Test windows string.
        let content = Content::new("first\r\nsecond\r\nthird".to_string());
        assert_eq!(content.body, "first\nsecond\nthird");

        // Test Unixstring.
        let content = Content::new("first\nsecond\nthird".to_string());
        assert_eq!(content.body, "first\nsecond\nthird");

        // Test BOM removal.
        let content = Content::new("\u{feff}first\nsecond\nthird".to_string());
        assert_eq!(content.body, "first\nsecond\nthird");

        // Test header extraction.
        let content = Content::new("\u{feff}---\nfirst\n---\nsecond\nthird".to_string());
        assert_eq!(content.header, "first");
        assert_eq!(content.body, "second\nthird");

        // Test header extraction without `\n` at the end.
        let content = Content::new("\u{feff}---\nfirst\n---".to_string());
        assert_eq!(content.header, "first");
        assert_eq!(content.body, "");

        // This fails to find the header.
        let content = Content::new("\u{feff}not ignored\n\n---\nfirst\n---".to_string());
        assert_eq!(content.header, "");
        assert_eq!(content.body, "not ignored\n\n---\nfirst\n---");
    }

    #[test]
    fn test_new_relax() {
        // The same a in the example above.
        let content = Content::new_relax("\u{feff}---\nfirst\n---".to_string());
        assert_eq!(content.header, "first");
        assert_eq!(content.body, "");

        // Here you can see the effect of relax.
        let content = Content::new_relax("\u{feff}ignored\n\n---\nfirst\n---".to_string());
        assert_eq!(content.header, "first");
        assert_eq!(content.body, "");
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
        let expected = ("first", "second\nthird");
        let result = Content::split(&input_stream, false);
        assert_eq!(result, expected);

        let input_stream = String::from("---\nfirst\n---\nsecond\nthird");
        let expected = ("first", "second\nthird");
        let result = Content::split(&input_stream, false);
        assert_eq!(result, expected);

        // Header is trimmed.
        let input_stream = String::from("---\n\nfirst\n\n---\nsecond\nthird");
        let expected = ("first", "second\nthird");
        let result = Content::split(&input_stream, false);
        assert_eq!(result, expected);

        // Body is kept as it is (not trimmed).
        let input_stream = String::from("---\nfirst\n---\n\nsecond\nthird\n");
        let expected = ("first", "\nsecond\nthird\n");
        let result = Content::split(&input_stream, false);
        assert_eq!(result, expected);

        let input_stream = String::from("\nsecond\nthird");
        let expected = ("", "\nsecond\nthird");
        let result = Content::split(&input_stream, false);
        assert_eq!(result, expected);

        let input_stream = String::from("");
        let expected = ("", "");
        let result = Content::split(&input_stream, false);
        assert_eq!(result, expected);

        let input_stream = String::from("\u{feff}\nsecond\nthird");
        let expected = ("", "\nsecond\nthird");
        let result = Content::split(&input_stream, false);
        assert_eq!(result, expected);

        let input_stream = String::from("\u{feff}");
        let expected = ("", "");
        let result = Content::split(&input_stream, false);
        assert_eq!(result, expected);
    }

    #[test]
    fn test_display_for_content() {
        let input = Content {
            s: "".to_string(),
            header: "first",
            body: "\nsecond\nthird\n",
        };
        let expected = "\u{feff}---\nfirst\n---\n\nsecond\nthird\n".to_string();
        assert_eq!(input.to_string(), expected);

        let input = Content {
            s: "".to_string(),
            header: "",
            body: "\nsecond\nthird\n",
        };
        let expected = "\nsecond\nthird\n".to_string();
        assert_eq!(input.to_string(), expected);

        let input = Content {
            s: "".to_string(),
            header: "",
            body: "",
        };
        let expected = "".to_string();
        assert_eq!(input.to_string(), expected);
    }
}
