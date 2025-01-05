//! Self referencing data structures to store the note's
//! content as a raw string.
use self_cell::self_cell;
use std::fmt;
use std::fmt::Debug;
use std::fs::create_dir_all;
use std::fs::read_to_string;
use std::fs::OpenOptions;
use std::io::Write;
use std::path::Path;
use substring::Substring;

use crate::error::InputStreamError;

/// As all text before the header marker `"---"` is ignored, this
/// constant limits the maximum number of characters that are skipped
/// before the header starts. In other words: the header
/// must start within the first `BEFORE_HEADER_MAX_IGNORED_CHARS`.
const BEFORE_HEADER_MAX_IGNORED_CHARS: usize = 1024;

/// This trait represents Tp-Note content.
/// The content is devided into header and body.
/// The header is the YAML meta data describing the body.
/// In some cases the header might be empty, e.g. when the data comes from
/// the clipboard (the `txt_clipboard` data might come with a header).
/// The body is flat UTF-8 markup formatted text, e.g. in
/// Markdown or in ReStructuredText.
/// A special case is HTML data in the body, originating from the HTML
/// clipboard. Here, the body always starts with an HTML start tag
/// (for details see the `html::HtmlStream` trait) and the header is always
/// empty.
///
/// The trait provides cheap access to the header with `header()`, the body
/// with `body()`, and the whole raw text with `as_str()`.
/// Implementers should cache the `header()` and `body()` function results in
/// oder to keep these as cheap as possible.
///
/// ```rust
/// use tpnote_lib::content::Content;
/// use tpnote_lib::content::ContentString;
/// let input = "---\ntitle: \"My note\"\n---\nMy body";
/// let c = ContentString::from(String::from(input));
///
/// assert_eq!(c.header(), r#"title: "My note""#);
/// assert_eq!(c.body(), r#"My body"#);
/// assert_eq!(c.as_str(), input);
///
/// // A test without front matter leads to an empty header:
/// let c = ContentString::from(String::from("No header"));
///
/// assert_eq!(c.header(), "");
/// assert_eq!(c.body(), "No header");
/// assert_eq!(c.as_str(), "No header");
/// ```
///
/// The `Content` trait allows to plug in your own storage back end if
/// `ContentString` does not suit you. In addtion to the example shown below,
/// you can overwrite `Content::open()` and `Content::save_as()` as well.
///
/// ```rust
/// use tpnote_lib::content::Content;
/// use std::string::String;
///
/// #[derive(Debug, Eq, PartialEq, Default)]
/// struct MyString(String);
/// impl Content for MyString {
///     /// This sample implementation may be too expensive.
///     /// Better precalculate this in `Self::from()`.
///     fn header(&self) -> &str {
///         Self::split(&self.as_str()).0
///     }
///     fn body(&self) -> &str {
///         Self::split(&self.as_str()).1
///     }
/// }
///
/// impl From<String> for MyString {
///     fn from(input: String) -> Self {
///         Self(input)    
///     }
/// }
///
/// impl AsRef<str> for MyString {
///     fn as_ref(&self) -> &str {
///         &self.0
///     }
/// }
///
/// let input = "---\ntitle: \"My note\"\n---\nMy body";
/// let s = MyString::from(input.to_string());
///
/// assert_eq!(s.header(), r#"title: "My note""#);
/// assert_eq!(s.body(), r#"My body"#);
/// assert_eq!(s.as_str(), input);
/// ```
pub trait Content: AsRef<str> + Debug + Eq + PartialEq + Default + From<String> {
    /// Reads the file at `path` and stores the content
    /// `Content`. Possible `\r\n` are replaced by `\n`.
    /// This trait has a default implementation, the empty content.
    ///
    /// ```rust
    /// use tpnote_lib::content::Content;
    /// use tpnote_lib::content::ContentString;
    /// use std::env::temp_dir;
    ///
    /// // Prepare test.
    /// let raw = "---\ntitle: \"My note\"\n---\nMy body";
    /// let notefile = temp_dir().join("20221030-hello -- world.md");
    /// let _ = std::fs::write(&notefile, raw.as_bytes());
    ///
    /// // Start test.
    /// let c = ContentString::open(&notefile).unwrap();
    ///
    /// assert_eq!(c.header(), r#"title: "My note""#);
    /// assert_eq!(c.body(), "My body");
    /// ```
    fn open(path: &Path) -> Result<Self, std::io::Error>
    where
        Self: Sized,
    {
        Ok(Self::from_string_with_cr(read_to_string(path)?))
    }

    /// Constructor that reads a structured document with a YAML header
    /// and body. All `\r\n` are converted to `\n` if there are any.
    /// If not, no memory allocation occurs and the buffer remains untouched.
    ///
    /// ```rust
    /// use tpnote_lib::content::Content;
    /// use tpnote_lib::content::ContentString;
    /// let c = ContentString::from_string_with_cr(String::from(
    ///     "---\r\ntitle: \"My note\"\r\n---\r\nMy\nbody\r\n"));
    ///
    /// assert_eq!(c.header(), r#"title: "My note""#);
    /// assert_eq!(c.body(), "My\nbody\n");
    ///
    /// // A test without front matter leads to an empty header:
    /// let c = ContentString::from(String::from("No header"));
    ///
    /// assert_eq!(c.borrow_dependent().header, "");
    /// assert_eq!(c.borrow_dependent().body, r#"No header"#);
    /// ```
    fn from_string_with_cr(input: String) -> Self {
        // Avoid allocating when there is nothing to do.
        let input = if input.find('\r').is_none() {
            // Forward without allocating.
            input
        } else {
            // We allocate here and do a lot of copying.
            input.replace("\r\n", "\n")
        };
        Self::from(input)
    }

    /// Return a reference to theader part in between `---`
    fn header(&self) -> &str;

    /// Return the body below the second `---`.
    fn body(&self) -> &str;

    /// Constructor that accepts and store HTML imput in the body.
    /// If the HTML input does not start with `<!DOCTYPE html...>` it is
    /// automatically prepended.
    /// If the input starts with another DOCTYPE than HTMl, return
    /// `InputStreamError::NonHtmlDoctype`.
    ///
    /// ```rust
    /// use tpnote_lib::content::Content;
    /// use tpnote_lib::content::ContentString;
    ///
    /// let c = ContentString::from_html(String::from(
    ///     "Some HTML content")).unwrap();
    /// assert_eq!(c.header(), "");
    /// assert_eq!(c.body(), "<!DOCTYPE html>Some HTML content");
    ///
    /// let c = ContentString::from_html(String::from(
    ///     "<!DOCTYPE html>Some HTML content")).unwrap();
    /// assert_eq!(c.header(), "");
    /// assert_eq!(c.body(), "<!DOCTYPE html>Some HTML content");
    ///
    /// let c = ContentString::from_html(String::from(
    ///     "<!DOCTYPE xml>Some HTML content"));
    /// assert!(c.is_err());
    /// ```
    fn from_html(input: String) -> Result<Self, InputStreamError> {
        use crate::html::HtmlString;
        let input = input.prepend_html_start_tag()?;
        Ok(Self::from(input))
    }

    /// Writes the note to disk with `new_file_path` as filename.
    /// If `new_file_path` contains missing directories, they will be
    /// created on the fly.
    ///
    /// ```rust
    /// use std::path::Path;
    /// use std::env::temp_dir;
    /// use std::fs;
    /// use tpnote_lib::content::Content;
    /// use tpnote_lib::content::ContentString;
    /// let c = ContentString::from(
    ///      String::from("prelude\n\n---\ntitle: \"My note\"\n---\nMy body"));
    /// let outfile = temp_dir().join("mynote.md");
    /// #[cfg(not(target_family = "windows"))]
    /// let expected = "\u{feff}prelude\n\n---\ntitle: \"My note\"\n---\nMy body\n";
    /// #[cfg(target_family = "windows")]
    /// let expected = "\u{feff}prelude\r\n\r\n---\r\ntitle: \"My note\"\r\n---\r\nMy body\r\n";
    ///
    /// c.save_as(&outfile).unwrap();
    /// let result = fs::read_to_string(&outfile).unwrap();
    ///
    /// assert_eq!(result, expected);
    /// fs::remove_file(&outfile);
    /// ```
    fn save_as(&self, new_file_path: &Path) -> Result<(), std::io::Error> {
        // Create missing directories, if there are any.
        create_dir_all(new_file_path.parent().unwrap_or_else(|| Path::new("")))?;

        let mut outfile = OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .open(new_file_path)?;

        log::trace!("Creating file: {:?}", new_file_path);
        write!(outfile, "\u{feff}")?;

        for l in self.as_str().lines() {
            write!(outfile, "{}", l)?;
            #[cfg(target_family = "windows")]
            write!(outfile, "\r")?;
            writeln!(outfile)?;
        }

        Ok(())
    }

    /// Accesses the whole content with all `---`.
    /// Contract: The content does not contain any `\r\n`.
    /// If your content contains `\r\n` use the
    /// `from_string_with_cr()` constructor.
    /// Possible BOM at the first position is not returned.
    fn as_str(&self) -> &str {
        self.as_ref().trim_start_matches('\u{feff}')
    }

    /// True if the header and body is empty.
    ///
    /// ```rust
    /// use tpnote_lib::content::Content;
    /// use tpnote_lib::content::ContentString;
    ///
    /// let c = ContentString::from(
    ///     "".to_string());
    /// assert_eq!(c.header(), "");
    /// assert_eq!(c.body(), "");
    /// assert!(c.is_empty());
    ///
    /// let c = ContentString::from(
    ///     "Some content".to_string());
    /// assert_eq!(c.header(), "");
    /// assert_eq!(c.body(), "Some content");
    /// assert!(!c.is_empty());
    ///
    /// let c = ContentString::from_html(
    ///     "".to_string()).unwrap();
    /// assert_eq!(c.header(), "");
    /// assert_eq!(c.body(), "<!DOCTYPE html>");
    /// assert!(c.is_empty());
    ///
    /// let c = ContentString::from_html(
    ///     "Some HTML content".to_string()).unwrap();
    /// assert_eq!(c.header(), "");
    /// assert_eq!(c.body(), "<!DOCTYPE html>Some HTML content");
    /// assert!(!c.is_empty());
    ///
    ///
    /// let c = ContentString::from(
    ///      String::from("---\ntitle: My note\n---\n"));
    /// assert_eq!(c.header(), "title: My note");
    /// assert_eq!(c.body(), "");
    /// assert!(!c.is_empty());
    /// ```
    fn is_empty(&self) -> bool {
        // Bring methods into scope. Overwrites `is_empty()`.
        use crate::html::HtmlStr;
        // `.is_empty_html()` is true for `""` or only `<!DOCTYPE html...>`.
        self.header().is_empty() && (self.body().is_empty_html())
    }

    /// Helper function that splits the content into header and body.
    /// The header, if present, is trimmed (`trim()`), the body
    /// is kept as it is.
    /// Any BOM (byte order mark) at the beginning is ignored.
    ///
    /// 1. Ignore `\u{feff}` if present
    /// 2. Ignore `---\n` or ignore all bytes until`\n\n---\n`,
    /// 3. followed by header bytes,
    /// 4. optionally followed by `\n`,
    /// 5. followed by `\n---\n` or `\n...\n`,
    /// 6. optionally followed by some `\t` and/or some ` `,
    /// 7. optionally followed by `\n`.
    ///
    /// The remaining bytes are the "body".
    ///
    /// Alternatively, a YAML metadata block may occur anywhere in the document, but if it is not
    /// at the beginning, it must be preceded by a blank line:
    /// 1. skip all text (BEFORE_HEADER_MAX_IGNORED_CHARS) until you find `"\n\n---"`
    /// 2. followed by header bytes,
    /// 3. same as above ...
    fn split(content: &str) -> (&str, &str) {
        // Bring in scope `HtmlString`.
        use crate::html::HtmlStr;
        // Remove BOM
        let content = content.trim_start_matches('\u{feff}');

        if content.is_empty() {
            return ("", "");
        };

        // If this is HTML content, leave the header empty.
        // TODO: In the future the header might be constructed from
        // the "meta" HTML fields. Though I am not sure if something meaningful
        // can be found in HTML clipboard meta data.
        if content.has_html_start_tag() {
            return ("", content);
        }

        const HEADER_START_TAG: &str = "---";
        let fm_start = if content.starts_with(HEADER_START_TAG) {
            // Found at first byte.
            HEADER_START_TAG.len()
        } else {
            const HEADER_START_TAG: &str = "\n\n---";
            if let Some(start) = content
                .substring(0, BEFORE_HEADER_MAX_IGNORED_CHARS)
                .find(HEADER_START_TAG)
                .map(|x| x + HEADER_START_TAG.len())
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

        const HEADER_END_TAG1: &str = "\n---";
        // Contract: next pattern must have the same length!
        const HEADER_END_TAG2: &str = "\n...";
        debug_assert_eq!(HEADER_END_TAG1.len(), HEADER_END_TAG2.len());
        const TAG_LEN: usize = HEADER_END_TAG1.len();

        let fm_end = content[fm_start..]
            .find(HEADER_END_TAG1)
            .or_else(|| content[fm_start..].find(HEADER_END_TAG2))
            .map(|x| x + fm_start);

        let fm_end = if let Some(n) = fm_end {
            n
        } else {
            return ("", content);
        };

        // We advance 4 because `"\n---"` has 4 bytes.
        let mut body_start = fm_end + TAG_LEN;

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
}

#[derive(Debug, Eq, PartialEq)]
/// Pointers belonging to the self referential struct `Content`.
pub struct ContentRef<'a> {
    /// Skip optional BOM and `"---" `in `s` until next `"---"`.
    /// When no `---` is found, this is empty.
    /// `header` is always trimmed.
    pub header: &'a str,
    /// Skip optional BOM and optional header and keep the rest.
    pub body: &'a str,
}

self_cell!(
/// Holds the notes content in a string and two string slices
/// `header`  and `body`.
/// This struct is self referential.
/// It deals with operating system specific handling of newlines.
/// The content of a note is stored as UTF-8 string with
/// one `\n` character as newline. If present, a Byte Order Mark
/// BOM is removed while reading with `new()`.
    pub struct ContentString {
        owner: String,

        #[covariant]
        dependent: ContentRef,
    }

    impl {Debug, Eq, PartialEq}
);

/// Constructor that parses a _Tp-Note_ document.
/// A valid document is UTF-8 encoded and starts with an optional
/// BOM (byte order mark) followed by `---`. When the start marker
/// `---` does not follow directly the BOM, it must be prepended
/// by an empty line. In this case all text before is ignored:
/// BOM + ignored text + empty line + `---`.
/// Contract: the input string does not contain `\r\n`. If
/// it may, use `Content::from_string_with_cr()` instead.
///
/// ```rust
/// use tpnote_lib::content::Content;
/// use tpnote_lib::content::ContentString;
/// let input = "---\ntitle: \"My note\"\n---\nMy body";
/// let c = ContentString::from(input.to_string());
///
/// assert_eq!(c.header(), r#"title: "My note""#);
/// assert_eq!(c.body(), "My body");
///
/// // A test without front matter leads to an empty header:
/// let c = ContentString::from("No header".to_string());
///
/// assert_eq!(c.header(), "");
/// assert_eq!(c.body(), "No header");
/// ```
impl From<String> for ContentString {
    /// Self referential. The constructor splits the content
    /// in header and body.
    fn from(input: String) -> Self {
        ContentString::new(input, |owner: &String| {
            let (header, body) = ContentString::split(owner);
            ContentRef { header, body }
        })
    }
}

/// Add `header()` and `body()` implementation.
impl Content for ContentString {
    /// Cheap access to the note's header.
    fn header(&self) -> &str {
        self.borrow_dependent().header
    }

    /// Cheap access to the note's body.
    fn body(&self) -> &str {
        self.borrow_dependent().body
    }
}

/// Default is the empty string.
impl Default for ContentString {
    fn default() -> Self {
        Self::from(String::new())
    }
}

/// Returns the whole raw content with header and body.
/// Possible `\r\n` in the input are replaced by `\n`.
impl AsRef<str> for ContentString {
    fn as_ref(&self) -> &str {
        self.borrow_owner()
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
impl fmt::Display for ContentString {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        std::fmt::Display::fmt(&self.borrow_dependent(), f)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_from_string_with_cr() {
        // Test windows string.
        let content = ContentString::from_string_with_cr("first\r\nsecond\r\nthird".to_string());
        assert_eq!(content.borrow_dependent().body, "first\nsecond\nthird");

        // Test Unix string.
        let content = ContentString::from_string_with_cr("first\nsecond\nthird".to_string());
        assert_eq!(content.borrow_dependent().body, "first\nsecond\nthird");

        // Test BOM removal.
        let content =
            ContentString::from_string_with_cr("\u{feff}first\nsecond\nthird".to_string());
        assert_eq!(content.borrow_dependent().body, "first\nsecond\nthird");
    }

    #[test]
    fn test_new() {
        // Test Unix string.
        let content = ContentString::from("first\nsecond\nthird".to_string());
        assert_eq!(content.borrow_dependent().body, "first\nsecond\nthird");

        // Test BOM removal.
        let content = ContentString::from("\u{feff}first\nsecond\nthird".to_string());
        assert_eq!(content.borrow_dependent().body, "first\nsecond\nthird");

        // Test header extraction.
        let content = ContentString::from("\u{feff}---\nfirst\n---\nsecond\nthird".to_string());
        assert_eq!(content.borrow_dependent().header, "first");
        assert_eq!(content.borrow_dependent().body, "second\nthird");

        // Test header extraction without `\n` at the end.
        let content = ContentString::from("\u{feff}---\nfirst\n---".to_string());
        assert_eq!(content.borrow_dependent().header, "first");
        assert_eq!(content.borrow_dependent().body, "");

        // Some skipped bytes.
        let content = ContentString::from("\u{feff}ignored\n\n---\nfirst\n---".to_string());
        assert_eq!(content.borrow_dependent().header, "first");
        assert_eq!(content.borrow_dependent().body, "");

        // This fails to find the header because the `---` comes to late.
        let mut s = "\u{feff}".to_string();
        s.push_str(&String::from_utf8(vec![b'X'; BEFORE_HEADER_MAX_IGNORED_CHARS]).unwrap());
        s.push_str("\n\n---\nfirst\n---\nsecond");
        let s_ = s.clone();
        let content = ContentString::from(s);
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
        let content = ContentString::from(s);
        assert_eq!(content.borrow_dependent().header, "first");
        assert_eq!(content.borrow_dependent().body, "second");
    }

    #[test]
    fn test_split() {
        // Document start marker is not followed by whitespace.
        let input_stream = String::from("---first\n---\nsecond\nthird");
        let expected = ("", "---first\n---\nsecond\nthird");
        let result = ContentString::split(&input_stream);
        assert_eq!(result, expected);

        // Document start marker is followed by whitespace.
        let input_stream = String::from("---\nfirst\n---\nsecond\nthird");
        let expected = ("first", "second\nthird");
        let result = ContentString::split(&input_stream);
        assert_eq!(result, expected);

        // Document start marker is followed by whitespace.
        let input_stream = String::from("---\tfirst\n---\nsecond\nthird");
        let expected = ("first", "second\nthird");
        let result = ContentString::split(&input_stream);
        assert_eq!(result, expected);

        // Document start marker is followed by whitespace.
        let input_stream = String::from("--- first\n---\nsecond\nthird");
        let expected = ("first", "second\nthird");
        let result = ContentString::split(&input_stream);
        assert_eq!(result, expected);

        // Header is trimmed.
        let input_stream = String::from("---\n\nfirst\n\n---\nsecond\nthird");
        let expected = ("first", "second\nthird");
        let result = ContentString::split(&input_stream);
        assert_eq!(result, expected);

        // Body is kept as it is (not trimmed).
        let input_stream = String::from("---\nfirst\n---\n\nsecond\nthird\n");
        let expected = ("first", "\nsecond\nthird\n");
        let result = ContentString::split(&input_stream);
        assert_eq!(result, expected);

        // Header end marker line is trimmed right.
        let input_stream = String::from("---\nfirst\n--- \t \n\nsecond\nthird\n");
        let expected = ("first", "\nsecond\nthird\n");
        let result = ContentString::split(&input_stream);
        assert_eq!(result, expected);

        let input_stream = String::from("\nsecond\nthird");
        let expected = ("", "\nsecond\nthird");
        let result = ContentString::split(&input_stream);
        assert_eq!(result, expected);

        let input_stream = String::from("");
        let expected = ("", "");
        let result = ContentString::split(&input_stream);
        assert_eq!(result, expected);

        let input_stream = String::from("\u{feff}\nsecond\nthird");
        let expected = ("", "\nsecond\nthird");
        let result = ContentString::split(&input_stream);
        assert_eq!(result, expected);

        let input_stream = String::from("\u{feff}");
        let expected = ("", "");
        let result = ContentString::split(&input_stream);
        assert_eq!(result, expected);

        let input_stream = String::from("[📽 2 videos]");
        let expected = ("", "[📽 2 videos]");
        let result = ContentString::split(&input_stream);
        assert_eq!(result, expected);

        let input_stream = "my prelude\n\n---\nmy header\n--- \nmy body\n";
        let expected = ("my header", "my body\n");
        let result = ContentString::split(input_stream);
        assert_eq!(result, expected);
    }

    #[test]
    fn test_display_for_content() {
        let expected = "\u{feff}---\nfirst\n---\n\nsecond\nthird\n".to_string();
        let input = ContentString::from(expected.clone());
        assert_eq!(input.to_string(), expected);

        let expected = "\nsecond\nthird\n".to_string();
        let input = ContentString::from(expected.clone());
        assert_eq!(input.to_string(), expected);

        let expected = "".to_string();
        let input = ContentString::from(expected.clone());
        assert_eq!(input.to_string(), expected);

        let expected = "\u{feff}---\nfirst\n---\n\nsecond\nthird\n".to_string();
        let input = ContentString::from(
            "\u{feff}ignored\n\n---\nfirst\n---\n\nsecond\nthird\n".to_string(),
        );
        assert_eq!(input.to_string(), expected);
    }
}
