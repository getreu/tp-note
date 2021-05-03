//! Deals with the note's content string.

use crate::error::FileError;
use core::marker::PhantomPinned;
use std::fmt;
use std::fs::OpenOptions;
use std::io::Write;
use std::path::Path;
use std::pin::Pin;

/// This is a newtype and thin wrapper around the note's content.
/// It deals with operating system specific handling of newlines.
#[derive(Debug, PartialEq)]
pub struct Content<'a> {
    /// The raw content as String, can be empty.
    s: String,
    /// Skip optional BOM and `"---" `in `s` until next `"---"`.
    /// When no `---` is found, this is empty.
    /// `header` is always trimmed.
    pub header: &'a str,
    /// Skip optional BOM and optional header and keep the rest.
    pub body: &'a str,
    /// Pin this struct.
    /// By making self-referential structs opt-out of Unpin, there is
    /// no (safe) way to get a &mut T from a Pin<Box<T>> type for them. As a result, their
    /// internal self-references are guaranteed to stay valid.
    _pin: PhantomPinned,
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

/// The content of a note is stored as UTF-8 string with
/// one `\n` character as newline. If present, a Byte Order Mark
/// BOM is removed while reading with `new()`.
impl<'a> Content<'a> {
    /// Constructor that reads a structured document with a YAML header
    /// and body.
    /// First `"---"` is required to start at the beginning of the `input`.
    /// Any BOM (byte order mark) at the beginning is ignored.
    /// On Windows machines it converts all `\r\n` to `\n`.
    /// When `relax==false` the header is only found when it starts
    /// at the beginning of the input. With `true` it can be placed
    /// everywhere to be found.
    pub fn new(input: String, relax: bool) -> Pin<Box<Self>> {
        let input = Self::remove_cr(input);
        let mut c = Box::pin(Content {
            s: input,
            header: "",
            body: "",
            _pin: PhantomPinned,
        });

        // Calculate the pointers (`str`).
        // The following is safe, because `split()` guarantees, that `header` and `body`
        // point into `c.s` (are a slice of).
        let c_s = unborrow!(&c.s);
        let (header, body) = Self::split(&c_s, relax);

        // Store the pointers.
        // The get_unchecked_mut function works on a Pin<&mut T> instead of a Pin<Box<T>>, so we
        // have to use the Pin::as_mut for converting the value before.
        let mut_ref = Pin::as_mut(&mut c);
        // Rationale:
        // 1. Requirement:
        //    > This function is unsafe. You must guarantee that you will never move the data out of
        //    the mutable reference you receive when you call this function, so that the invariants on
        //    the Pin type can be upheld.
        // 2. The following is safe, because I just reassign a pointer and do not move any data.
        unsafe {
            Pin::get_unchecked_mut(mut_ref).header = header;
        }
        // Same here.
        let mut_ref = Pin::as_mut(&mut c);
        unsafe {
            Pin::get_unchecked_mut(mut_ref).body = body;
        }
        c
    }

    /// True if the `Content` is empty.
    pub fn is_empty(&self) -> bool {
        self.s.is_empty()
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
    /// To accept a "header" (`relax==false`):
    /// 1. the document must start with `"---"`,
    /// 2. followed by header bytes,
    /// 3. optionally followed by `"\n",
    /// 4. followed by `"---"` or `"..."`,
    /// 5. optionally followed by some `"\t"` and/or some `" "`,
    /// 5. optionally followed by `"\n"`.
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

        let pattern = "---";
        let fm_start = if content.starts_with(pattern) {
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
    pub fn write_to_disk(self: &Pin<Box<Self>>, new_file_path: &Path) -> Result<(), FileError> {
        let outfile = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&new_file_path);
        match outfile {
            Ok(mut outfile) => {
                log::trace!("Creating file: {:?}", new_file_path);
                write!(outfile, "\u{feff}")?;
                if !self.header.is_empty() {
                    write!(outfile, "---")?;
                    #[cfg(target_family = "windows")]
                    write!(outfile, "\r")?;
                    writeln!(outfile)?;
                    for l in self.header.lines() {
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
                for l in self.body.lines() {
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
    use super::*;

    #[test]
    fn test_new() {
        // Test windows string.
        let content = Content::new("first\r\nsecond\r\nthird".to_string(), false);
        assert_eq!(content.body, "first\nsecond\nthird");

        // Test Unix string.
        let content = Content::new("first\nsecond\nthird".to_string(), false);
        assert_eq!(content.body, "first\nsecond\nthird");

        // Test BOM removal.
        let content = Content::new("\u{feff}first\nsecond\nthird".to_string(), false);
        assert_eq!(content.body, "first\nsecond\nthird");

        // Test header extraction.
        let content = Content::new("\u{feff}---\nfirst\n---\nsecond\nthird".to_string(), false);
        assert_eq!(content.header, "first");
        assert_eq!(content.body, "second\nthird");

        // Test header extraction without `\n` at the end.
        let content = Content::new("\u{feff}---\nfirst\n---".to_string(), false);
        assert_eq!(content.header, "first");
        assert_eq!(content.body, "");

        // This fails to find the header.
        let content = Content::new("\u{feff}not ignored\n\n---\nfirst\n---".to_string(), false);
        assert_eq!(content.header, "");
        assert_eq!(content.body, "not ignored\n\n---\nfirst\n---");
    }

    #[test]
    fn test_new_relax() {
        // The same a in the example above.
        let content = Content::new("\u{feff}---\nfirst\n---".to_string(), true);
        assert_eq!(content.header, "first");
        assert_eq!(content.body, "");

        // Here you can see the effect of relax.
        let content = Content::new("\u{feff}ignored\n\n---\nfirst\n---".to_string(), true);
        assert_eq!(content.header, "first");
        assert_eq!(content.body, "");
    }

    #[test]
    fn test_split() {
        // Document start marker is not followed by whitespace.
        let input_stream = String::from("---first\n---\nsecond\nthird");
        let expected = ("", "---first\n---\nsecond\nthird");
        let result = Content::split(&input_stream, false);
        assert_eq!(result, expected);

        // Document start marker is followed by whitespace.
        let input_stream = String::from("---\nfirst\n---\nsecond\nthird");
        let expected = ("first", "second\nthird");
        let result = Content::split(&input_stream, false);
        assert_eq!(result, expected);

        // Document start marker is followed by whitespace.
        let input_stream = String::from("---\tfirst\n---\nsecond\nthird");
        let expected = ("first", "second\nthird");
        let result = Content::split(&input_stream, false);
        assert_eq!(result, expected);

        // Document start marker is followed by whitespace.
        let input_stream = String::from("--- first\n---\nsecond\nthird");
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

        // Header end marker line is trimmed right.
        let input_stream = String::from("---\nfirst\n--- \t \n\nsecond\nthird\n");
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

        let input_stream = String::from("[📽 2 videos]");
        let expected = ("", "[📽 2 videos]");
        let result = Content::split(&input_stream, false);
        assert_eq!(result, expected);
    }

    #[test]
    fn test_display_for_content() {
        let input = Content {
            s: "".to_string(),
            header: "first",
            body: "\nsecond\nthird\n",
            _pin: PhantomPinned,
        };
        let expected = "\u{feff}---\nfirst\n---\n\nsecond\nthird\n".to_string();
        assert_eq!(input.to_string(), expected);

        let input = Content {
            s: "".to_string(),
            header: "",
            body: "\nsecond\nthird\n",
            _pin: PhantomPinned,
        };
        let expected = "\nsecond\nthird\n".to_string();
        assert_eq!(input.to_string(), expected);

        let input = Content {
            s: "".to_string(),
            header: "",
            body: "",
            _pin: PhantomPinned,
        };
        let expected = "".to_string();
        assert_eq!(input.to_string(), expected);
    }
}
