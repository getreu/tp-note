//! Deals with the note's content string.

use crate::error::FileError;
use self_cell::self_cell;
use std::fmt;
use std::fs::create_dir_all;
use std::fs::OpenOptions;
use std::io::Write;
use std::path::Path;

/// As all text before the header marker `"---"` is ignored, this
/// constant limits the maximum number of characters that are skipped
/// before the header starts. In other words: the header
/// must start within the first `BEFORE_HEADER_MAX_IGNORED_CHARS`.
const BEFORE_HEADER_MAX_IGNORED_CHARS: usize = 1024;

/// The content of a note is stored as UTF-8 string with
/// one `\n` character as newline. If present, a Byte Order Mark
/// BOM is removed while reading with `new()`.
#[derive(Debug, Eq, PartialEq)]
pub struct ContentRef<'a> {
    /// Skip optional BOM and `"---" `in `s` until next `"---"`.
    /// When no `---` is found, this is empty.
    /// `header` is always trimmed.
    pub header: &'a str,
    /// Skip optional BOM and optional header and keep the rest.
    pub body: &'a str,
}

self_cell!(
/// This is a newtype and thin wrapper around the note's content.
/// It deals with operating system specific handling of newlines.
    pub struct Content {
        owner: String,

        #[covariant]
        dependent: ContentRef,
    }

    impl {Debug, Eq, PartialEq}
);

/// The content of a note is stored as UTF-8 string with
/// one `\n` character as newline. If present, a Byte Order Mark
/// BOM is removed while reading with `new()`.
impl<'a> Content {
    /// Constructor that parses a _Tp-Note_ document.
    /// A valid document is UTF-8 encoded and starts with an optional
    /// BOM (byte order mark) followed by "---". When the startmarker
    /// "---" does not follow directly the BOM, it must be prepended
    /// by an empty line. In this case all text before is ignored:
    /// BOM + ignored text + empty line + "---".
    pub fn from(input: String) -> Self {
        Content::new(input, |owner: &String| {
            let (header, body) = Content::split(owner);
            ContentRef { header, body }
        })
    }

    /// Constructor that reads a structured document with a YAML header
    /// and body.
    ///
    /// This converts all `\r\n` to `\n`.
    pub fn from_input_with_cr(input: String) -> Self {
        let input = Self::remove_cr(input);

        Content::from(input)
    }

    /// True if the `Content` is empty.
    pub fn is_empty(&self) -> bool {
        self.borrow_owner().is_empty()
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

    /// Helper function that splits the content into header and body.
    /// The header, if present, is trimmed (`trim()`), the body
    /// is kept as it is.
    /// Any BOM (byte order mark) at the beginning is ignored.
    ///
    /// 1. The document must start with `"---"`
    /// 2. followed by header bytes,
    /// 3. optionally followed by `"\n",
    /// 4. followed by `"---"` or `"..."`,
    /// 5. optionally followed by some `"\t"` and/or some `" "`,
    /// 5. optionally followed by `"\n"`.
    /// The remaining bytes are "content".
    ///
    /// Alternatively, a YAML metadata block may occur anywhere in the document, but if it is not
    /// at the beginning, it must be preceded by a blank line:
    /// 1. skip all text (BEFORE_HEADER_MAX_IGNORED_CHARS) until you find `"\n\n---"`
    /// 2. followed by header bytes,
    /// 3. same as above ...
    fn split(content: &'a str) -> (&'a str, &'a str) {
        // Remove BOM
        let content = content.trim_start_matches('\u{feff}');

        if content.is_empty() {
            return ("", "");
        };

        let pattern = "---";
        let fm_start = if content.starts_with(pattern) {
            // Found at first byte.
            pattern.len()
        } else {
            let pattern = "\n\n---";
            if let Some(start) = content
                .chars()
                .take(BEFORE_HEADER_MAX_IGNORED_CHARS)
                .collect::<String>()
                .find(pattern)
                .map(|x| x + pattern.len())
            {
                // Found just before `start`!
                start
            } else {
                // Not found.
                return ("", content);
            }
        };

        // The first character after the document start marker
        // must be a whitespace.
        if !content[fm_start..]
            .chars()
            .next()
            // If none, make test fail.
            .unwrap_or('x')
            .is_whitespace()
        {
            return ("", content);
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

        // Skip spaces and tabs followed by one optional newline.
        while let Some(c) = content[body_start..].chars().next() {
            if c == ' ' || c == '\t' {
                body_start += 1;
            } else {
                // Skip exactly one newline, if there is at least one.
                if c == '\n' {
                    body_start += 1;
                }
                // Exit loop.
                break;
            };
        }

        (content[fm_start..fm_end].trim(), &content[body_start..])
    }

    /// Writes the note to disk with `new_file_path`-filename.
    pub fn write_to_disk(&self, new_file_path: &Path) -> Result<(), FileError> {
        // Create missing directories, if there are any.
        create_dir_all(new_file_path.parent().unwrap_or_else(|| Path::new("")))?;

        let outfile = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&new_file_path);
        match outfile {
            Ok(mut outfile) => {
                log::trace!("Creating file: {:?}", new_file_path);
                write!(outfile, "\u{feff}")?;
                if !self.borrow_dependent().header.is_empty() {
                    write!(outfile, "---")?;
                    #[cfg(target_family = "windows")]
                    write!(outfile, "\r")?;
                    writeln!(outfile)?;
                    for l in self.borrow_dependent().header.lines() {
                        write!(outfile, "{}", l)?;
                        #[cfg(target_family = "windows")]
                        write!(outfile, "\r")?;
                        writeln!(outfile)?;
                    }
                    write!(outfile, "---")?;
                    #[cfg(target_family = "windows")]
                    write!(outfile, "\r")?;
                    writeln!(outfile)?;
                };
                for l in self.borrow_dependent().body.lines() {
                    write!(outfile, "{}", l)?;
                    #[cfg(target_family = "windows")]
                    write!(outfile, "\r")?;
                    writeln!(outfile)?;
                }
            }
            Err(e) => {
                return Err(FileError::Write {
                    path: new_file_path.to_path_buf(),
                    source_str: e.to_string(),
                });
            }
        }

        Ok(())
    }
}

/// Concatenates the header and the body and prints the content.
/// This function is expensive as it involves copying the
/// whole content.
impl<'a> fmt::Display for ContentRef<'a> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = if self.header.is_empty() {
            self.body.to_string()
        } else {
            format!("\u{feff}---\n{}\n---\n{}", &self.header, &self.body)
        };
        write!(f, "{}", s)
    }
}

/// Delegates the printing to `Display for ContentRef`.
impl fmt::Display for Content {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.borrow_dependent().fmt(f)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_from_input_with_cr() {
        // Test windows string.
        let content = Content::from_input_with_cr("first\r\nsecond\r\nthird".to_string());
        assert_eq!(content.borrow_dependent().body, "first\nsecond\nthird");

        // Test Unix string.
        let content = Content::from_input_with_cr("first\nsecond\nthird".to_string());
        assert_eq!(content.borrow_dependent().body, "first\nsecond\nthird");

        // Test BOM removal.
        let content = Content::from_input_with_cr("\u{feff}first\nsecond\nthird".to_string());
        assert_eq!(content.borrow_dependent().body, "first\nsecond\nthird");
    }

    #[test]
    fn test_new() {
        // Test Unix string.
        let content = Content::from("first\nsecond\nthird".to_string());
        assert_eq!(content.borrow_dependent().body, "first\nsecond\nthird");

        // Test BOM removal.
        let content = Content::from("\u{feff}first\nsecond\nthird".to_string());
        assert_eq!(content.borrow_dependent().body, "first\nsecond\nthird");

        // Test header extraction.
        let content = Content::from("\u{feff}---\nfirst\n---\nsecond\nthird".to_string());
        assert_eq!(content.borrow_dependent().header, "first");
        assert_eq!(content.borrow_dependent().body, "second\nthird");

        // Test header extraction without `\n` at the end.
        let content = Content::from("\u{feff}---\nfirst\n---".to_string());
        assert_eq!(content.borrow_dependent().header, "first");
        assert_eq!(content.borrow_dependent().body, "");

        // Some skipped bytes.
        let content = Content::from("\u{feff}ignored\n\n---\nfirst\n---".to_string());
        assert_eq!(content.borrow_dependent().header, "first");
        assert_eq!(content.borrow_dependent().body, "");

        // This fails to find the header because the `---` comes to late.
        let mut s = "\u{feff}".to_string();
        s.push_str(&String::from_utf8(vec![b'X'; BEFORE_HEADER_MAX_IGNORED_CHARS]).unwrap());
        s.push_str("\n\n---\nfirst\n---\nsecond");
        let s_ = s.clone();
        let content = Content::from(s);
        assert_eq!(content.borrow_dependent().header, "");
        assert_eq!(content.borrow_dependent().body, &s_[3..]);

        // This finds the header.
        let mut s = "\u{feff}".to_string();
        s.push_str(
            &String::from_utf8(vec![
                b'X';
                BEFORE_HEADER_MAX_IGNORED_CHARS - "\n\n---".len()
            ])
            .unwrap(),
        );
        s.push_str("\n\n---\nfirst\n---\nsecond");
        let content = Content::from(s);
        assert_eq!(content.borrow_dependent().header, "first");
        assert_eq!(content.borrow_dependent().body, "second");
    }

    #[test]
    fn test_split() {
        // Document start marker is not followed by whitespace.
        let input_stream = String::from("---first\n---\nsecond\nthird");
        let expected = ("", "---first\n---\nsecond\nthird");
        let result = Content::split(&input_stream);
        assert_eq!(result, expected);

        // Document start marker is followed by whitespace.
        let input_stream = String::from("---\nfirst\n---\nsecond\nthird");
        let expected = ("first", "second\nthird");
        let result = Content::split(&input_stream);
        assert_eq!(result, expected);

        // Document start marker is followed by whitespace.
        let input_stream = String::from("---\tfirst\n---\nsecond\nthird");
        let expected = ("first", "second\nthird");
        let result = Content::split(&input_stream);
        assert_eq!(result, expected);

        // Document start marker is followed by whitespace.
        let input_stream = String::from("--- first\n---\nsecond\nthird");
        let expected = ("first", "second\nthird");
        let result = Content::split(&input_stream);
        assert_eq!(result, expected);

        // Header is trimmed.
        let input_stream = String::from("---\n\nfirst\n\n---\nsecond\nthird");
        let expected = ("first", "second\nthird");
        let result = Content::split(&input_stream);
        assert_eq!(result, expected);

        // Body is kept as it is (not trimmed).
        let input_stream = String::from("---\nfirst\n---\n\nsecond\nthird\n");
        let expected = ("first", "\nsecond\nthird\n");
        let result = Content::split(&input_stream);
        assert_eq!(result, expected);

        // Header end marker line is trimmed right.
        let input_stream = String::from("---\nfirst\n--- \t \n\nsecond\nthird\n");
        let expected = ("first", "\nsecond\nthird\n");
        let result = Content::split(&input_stream);
        assert_eq!(result, expected);

        let input_stream = String::from("\nsecond\nthird");
        let expected = ("", "\nsecond\nthird");
        let result = Content::split(&input_stream);
        assert_eq!(result, expected);

        let input_stream = String::from("");
        let expected = ("", "");
        let result = Content::split(&input_stream);
        assert_eq!(result, expected);

        let input_stream = String::from("\u{feff}\nsecond\nthird");
        let expected = ("", "\nsecond\nthird");
        let result = Content::split(&input_stream);
        assert_eq!(result, expected);

        let input_stream = String::from("\u{feff}");
        let expected = ("", "");
        let result = Content::split(&input_stream);
        assert_eq!(result, expected);

        let input_stream = String::from("[📽 2 videos]");
        let expected = ("", "[📽 2 videos]");
        let result = Content::split(&input_stream);
        assert_eq!(result, expected);
    }

    #[test]
    fn test_display_for_content() {
        let expected = "\u{feff}---\nfirst\n---\n\nsecond\nthird\n".to_string();
        let input = Content::from(expected.clone());
        assert_eq!(input.to_string(), expected);

        let expected = "\nsecond\nthird\n".to_string();
        let input = Content::from(expected.clone());
        assert_eq!(input.to_string(), expected);

        let expected = "".to_string();
        let input = Content::from(expected.clone());
        assert_eq!(input.to_string(), expected);

        let expected = "\u{feff}---\nfirst\n---\n\nsecond\nthird\n".to_string();
        let input =
            Content::from("\u{feff}ignored\n\n---\nfirst\n---\n\nsecond\nthird\n".to_string());
        assert_eq!(input.to_string(), expected);
    }
}
