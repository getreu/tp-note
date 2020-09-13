//! Extends the built-in Tera filters.
extern crate sanitize_filename_reader_friendly;
use crate::config::Hyperlink;
use crate::config::CFG;
use lazy_static::lazy_static;
use sanitize_filename_reader_friendly::sanitize;
use std::collections::HashMap;
use std::hash::BuildHasher;
use std::ops::Deref;
use std::path::Path;
use std::path::PathBuf;
use tera::{to_value, try_get_value, Result as TeraResult, Tera, Value};

/// Filter parameter of the `cut_filter()` that limits the maximum length of template variables,
/// which are usually used to in the note's front matter as title. For example: the title should
/// not be too long, because it will end up as part of the file-name when the note is saved to
/// disk. Filenames of some operating systems are limited to 255 bytes.
#[cfg(not(test))]
const CUT_LEN_MAX: usize = 200;
#[cfg(test)]
const CUT_LEN_MAX: usize = 10;

lazy_static! {
/// Tera object with custom functions registered.
    pub static ref TERA: Tera = {
        let mut tera = Tera::default();
        tera.register_filter("sanit", sanit_filter);
        tera.register_filter("linkname", linkname_filter);
        tera.register_filter("linkurl", linkurl_filter);
        tera.register_filter("heading", heading_filter);
        tera.register_filter("cut", cut_filter);
        tera.register_filter("tag", tag_filter);
        tera.register_filter("stem", stem_filter);
        tera.register_filter("ext", ext_filter);
        tera.register_filter("prepend_dot", prepend_dot_filter);
        tera
    };
}

/// Add a new filter to Tera templates:
/// `sanit` or `sanit()` sanitizes a string so that it can be used
/// to assemble filenames or paths.
/// In addition, `sanit(alpha=true)` prepends an apostroph when the result
/// starts with a number. This way we guaranty that the filename
/// never starts with a number. We do not allow this, to be able
/// to distinguish reliably the sort tag from the filename.
pub fn sanit_filter<S: BuildHasher>(
    value: &Value,
    args: &HashMap<String, Value, S>,
) -> TeraResult<Value> {
    let p = try_get_value!("sanit", "value", String, value);

    let alpha_required = match args.get("alpha") {
        Some(val) => try_get_value!("sanit", "alpha", bool, val),
        None => false,
    };

    let mut filtered = sanitize(&p);

    if alpha_required {
        let first_char = filtered.chars().next();
        if let Some(c) = first_char {
            if c.is_numeric() {
                filtered.insert(0, '\'');
            }
        };
    };

    Ok(to_value(&filtered).unwrap())
}

/// A Tera filter that searches for the first Markdown link in the input stream and returns the
/// link's name.
pub fn linkname_filter<S: BuildHasher>(
    value: &Value,
    _args: &HashMap<String, Value, S>,
) -> TeraResult<Value> {
    let p = try_get_value!("linkname", "value", String, value);

    let hyperlink = Hyperlink::new(&p).unwrap_or_default();

    Ok(to_value(&hyperlink.name).unwrap())
}

/// A Tera filter that searches for the first Markdown link in the input stream and returns the
/// link's URL.
pub fn linkurl_filter<S: BuildHasher>(
    value: &Value,
    _args: &HashMap<String, Value, S>,
) -> TeraResult<Value> {
    let p = try_get_value!("linkname", "value", String, value);

    let hyperlink = Hyperlink::new(&p).unwrap_or_default();

    Ok(to_value(&hyperlink.url).unwrap())
}

/// A Tera filter that truncates the input stream and returns the
/// max `CUT_LEN_MAX` bytes of valid UTF-8.
pub fn cut_filter<S: BuildHasher>(
    value: &Value,
    _args: &HashMap<String, Value, S>,
) -> TeraResult<Value> {
    let p = try_get_value!("cut", "value", String, value);

    let mut short = String::new();
    for i in (0..CUT_LEN_MAX).rev() {
        if let Some(s) = p.get(..i) {
            short = s.to_string();
            break;
        }
    }

    Ok(to_value(&short).unwrap())
}

/// A Tera filter that return the first line or the first sentence of the input stream.
pub fn heading_filter<S: BuildHasher>(
    value: &Value,
    _args: &HashMap<String, Value, S>,
) -> TeraResult<Value> {
    let p = try_get_value!("heading", "value", String, value);
    let p = p.trim_start();

    // Find the first heading, can finish with `. `, `.\n` or `.\r\n` on Windows.
    let mut index = p.len();

    if let Some(i) = p.find(". ") {
        if i < index {
            index = i;
        }
    }
    if let Some(i) = p.find(".\n") {
        if i < index {
            index = i;
        }
    }
    if let Some(i) = p.find(".\r\n") {
        if i < index {
            index = i;
        }
    }
    if let Some(i) = p.find('!') {
        if i < index {
            index = i;
        }
    }
    if let Some(i) = p.find('?') {
        if i < index {
            index = i;
        }
    }
    if let Some(i) = p.find("\n\n") {
        if i < index {
            index = i;
        }
    }
    if let Some(i) = p.find("\r\n\r\n") {
        if i < index {
            index = i;
        }
    }
    let content_heading = p[0..index].to_string();

    Ok(to_value(content_heading).unwrap())
}

/// A Tera filter that takes a path and extracts the tag of the filename.
pub fn tag_filter<S: BuildHasher>(
    value: &Value,
    _args: &HashMap<String, Value, S>,
) -> TeraResult<Value> {
    let p = try_get_value!("tag", "value", String, value);

    let (tag, _, _, _) = disassemble_filename(Path::new(&p));

    Ok(to_value(&tag).unwrap())
}

/// A Tera filter that takes a path and extracts its file stem.
pub fn stem_filter<S: BuildHasher>(
    value: &Value,
    _args: &HashMap<String, Value, S>,
) -> TeraResult<Value> {
    let p = try_get_value!("stem", "value", String, value);

    let (_, stem, _, _) = disassemble_filename(Path::new(&p));

    Ok(to_value(&stem).unwrap())
}

/// A Tera filter that prepends a dot when stream not empty.
pub fn prepend_dot_filter<S: BuildHasher>(
    value: &Value,
    _args: &HashMap<String, Value, S>,
) -> TeraResult<Value> {
    let p = try_get_value!("prepend_dot", "value", String, value);

    let mut prepend_dot = String::new();

    if !p.is_empty() {
        prepend_dot.push('.');
        prepend_dot.push_str(&p);
    };

    Ok(to_value(&prepend_dot).unwrap())
}

/// A Tera filter that takes a path and extracts its file extension.
pub fn ext_filter<S: BuildHasher>(
    value: &Value,
    _args: &HashMap<String, Value, S>,
) -> TeraResult<Value> {
    let p = try_get_value!("ext", "value", String, value);

    let ext = Path::new(&p)
        .extension()
        .unwrap_or_default()
        .to_str()
        .unwrap_or_default();

    Ok(to_value(&ext).unwrap())
}

pub fn filename_exclude_copy_counter_eq(p1: &Path, p2: &Path) -> bool {
    let (sort_tag1, stem1, _, ext1) = disassemble_filename(p1);
    let (sort_tag2, stem2, _, ext2) = disassemble_filename(p2);
    sort_tag1 == sort_tag2 && stem1 == stem2 && ext1 == ext2
}

/// Helper function that decomposes a fully qualified path name
/// into (parent_dir, sort_tag, file_stem_without_sort_tag, extension).
pub fn disassemble_filename<'a>(p: &Path) -> (&str, &str, &str, &str) {
    let file_stem = p
        .file_stem()
        .unwrap_or_default()
        .to_str()
        .unwrap_or_default();

    let stem_copy_counter =
        file_stem.trim_start_matches(|c: char| c.is_numeric() || c == '-' || c == '_');

    let sort_tag = &file_stem[0..file_stem.len() - stem_copy_counter.len()];

    let stem = remove_copy_counter(stem_copy_counter);

    let copy_counter = &stem_copy_counter[stem.len()..];

    let extension = p
        .extension()
        .unwrap_or_default()
        .to_str()
        .unwrap_or_default();
    (sort_tag, stem, copy_counter, extension)
}

/// Concatenates the 3 parameters.
pub fn assemble_filename(
    sort_tag: &str,
    stem: &str,
    copy_counter: &str,
    extension: &str,
) -> String {
    // Assemble path.
    let mut filename = sort_tag.to_string();
    filename.push_str(stem);
    filename.push_str(copy_counter);
    if !extension.is_empty() {
        filename.push('.');
        filename.push_str(extension);
    };
    filename
}

/// Helper function that trims the pattern `_-n-_` at the end of string matching
/// `*_-n-_`, where `n` is an integer.
/// When the pattern is not found return the whole string.
/// Do the same if the string ends with `-_n_-`.
#[inline]
pub fn remove_copy_counter(tag: &str) -> &str {
    // Strip `sepsepend` at the end.
    let tag1 = if let Some(t) = tag.strip_suffix(&CFG.copy_counter_closing_brackets) {
        t
    } else {
        return tag;
    };
    // Now strip numbers.
    let tag2 = tag1.trim_end_matches(|c: char| c.is_numeric());
    if tag2.len() == tag1.len() {
        return tag;
    };
    // And finally strip `sepsepstart`.
    let tag3 = if let Some(t) = tag2.strip_suffix(&CFG.copy_counter_opening_brackets) {
        t
    } else {
        return tag;
    };

    return tag3;
}

/// When the string ends with `_` append the string `_-n-_`, where `n` is an integer.
/// Otherwise append `-_n_-`.
/// Before appending, remove all trailing `_` and `-`.
#[inline]
pub fn append_copy_counter(tag: &str, n: usize) -> String {
    // Remove more separators, if they exist.
    let mut tag = tag.trim_end_matches(|c| c == '_' || c == '-').to_string();
    // Append sort-tag extension.
    tag.push_str(&CFG.copy_counter_opening_brackets);
    tag.push_str(&n.to_string());
    tag.push_str(&CFG.copy_counter_closing_brackets);
    tag
}

/// Tiny wrapper around Tera-context with some additional information.
#[derive(Debug, PartialEq)]
pub struct ContextWrapper {
    // Collection of substitution variables.
    ct: tera::Context,
    // The note's directory path on disk.
    pub fqpn: PathBuf,
}

/// A thin wrapper around `tera::Context` storing some additional
/// information.
impl ContextWrapper {
    pub fn new() -> Self {
        Self {
            ct: tera::Context::new(),
            fqpn: PathBuf::new(),
        }
    }

    /// Function that forwards a `kay-value` to the encapsulated `
    /// tera::Context::insert()` function.
    pub fn insert(&mut self, key: &str, val: &str) {
        // The first version is the unmodified variable `<key>` with original <val>.
        self.ct.insert(key, &val);
    }
}

/// Auto-dereference for convenient access to `tera::Content`.
impl Deref for ContextWrapper {
    type Target = tera::Context;

    fn deref(&self) -> &Self::Target {
        &self.ct
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use tera::to_value;

    #[test]
    fn test_sanit_filter() {
        let result = sanit_filter(
            &to_value(&".# Strange filename? Yes.").unwrap(),
            &HashMap::new(),
        );
        assert!(result.is_ok());
        assert_eq!(
            result.unwrap(),
            to_value(&"Strange filename_ Yes.").unwrap()
        );
    }

    #[test]
    fn test_sanit_filter_alpha() {
        let mut args = HashMap::new();
        args.insert("alpha".to_string(), to_value(true).unwrap());
        let result = sanit_filter(&to_value(&"1. My first: chapter").unwrap(), &args);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), to_value(&"'1. My first_ chapter").unwrap());
    }
    #[test]
    fn test_linkname_linkurl_filter() {
        let args = HashMap::new();
        // Test Markdown link in clipboard.
        let input = "xxx[Jens Getreu's blog](https://blog.getreu.net)";
        let output_ln = linkname_filter(&to_value(&input).unwrap(), &args).unwrap_or_default();
        assert_eq!("Jens Getreu's blog", output_ln);
        let output_lu = linkurl_filter(&to_value(&input).unwrap(), &args).unwrap_or_default();
        assert_eq!("https://blog.getreu.net", output_lu);

        // Test non-link string in clipboard.
        let input = "Tp-Note helps you to quickly get\
            started writing notes.";
        let output_ln = linkname_filter(&to_value(&input).unwrap(), &args).unwrap_or_default();
        assert_eq!("", output_ln);
        let output_lu = linkurl_filter(&to_value(&input).unwrap(), &args).unwrap_or_default();
        assert_eq!("", output_lu);
    }
    #[test]
    fn test_cut_filter() {
        let args = HashMap::new();
        // Test Markdown link in clipboard.
        let input = "Jens Getreu's blog";
        let output = cut_filter(&to_value(&input).unwrap(), &args).unwrap_or_default();
        assert_eq!("Jens Getr", output);
    }
    #[test]
    fn test_heading_filter() {
        let args = HashMap::new();

        //
        // Test find first sentence.
        let input = "N.ote.\nIt helps. Get quickly\
            started writing notes.";
        let output = heading_filter(&to_value(&input).unwrap(), &args).unwrap_or_default();
        // This string is shortened.
        assert_eq!("N.ote", output);

        //
        // Test find first sentence (Windows)
        let input = "N.ote.\r\nIt helps. Get quickly\
            started writing notes.";
        let output = heading_filter(&to_value(&input).unwrap(), &args).unwrap_or_default();
        // This string is shortened.
        assert_eq!("N.ote", output);

        //
        // Test find heading
        let input = "N.ote\n\nIt helps. Get quickly\
            started writing notes.";
        let output = heading_filter(&to_value(&input).unwrap(), &args).unwrap_or_default();
        // This string is shortened.
        assert_eq!("N.ote", output);

        //
        // Test find heading (Windows)
        let input = "N.ote\r\n\r\nIt helps. Get quickly\
            started writing notes.";
        let output = heading_filter(&to_value(&input).unwrap(), &args).unwrap_or_default();
        // This string is shortened.
        assert_eq!("N.ote", output);

        //
        // Test trim whitespace
        let input = "\r\n\r\n  \tIt helps. Get quickly\
            started writing notes.";
        let output = heading_filter(&to_value(&input).unwrap(), &args).unwrap_or_default();
        // This string is shortened.
        assert_eq!("It helps", output);
    }
    #[test]
    fn test_file_filter() {
        let args = HashMap::new();
        //
        //
        // Test file stem.
        let input =
            "/usr/local/WEB-SERVER-CONTENT/blog.getreu.net/projects/tp-note/20200908-My file.md";
        let output = stem_filter(&to_value(&input).unwrap(), &args).unwrap_or_default();
        assert_eq!("My file", output);

        let input =
            "/usr/local/WEB-SERVER-CONTENT/blog.getreu.net/projects/tp-note/20200908-My dir/";
        let output = stem_filter(&to_value(&input).unwrap(), &args).unwrap_or_default();
        assert_eq!("My dir", output);
        //
        //
        // Test file tag.
        let input =
            "/usr/local/WEB-SERVER-CONTENT/blog.getreu.net/projects/tp-note/20200908-My file.md";
        let output = tag_filter(&to_value(&input).unwrap(), &args).unwrap_or_default();
        assert_eq!("20200908-", output);

        let input =
            "/usr/local/WEB-SERVER-CONTENT/blog.getreu.net/projects/tp-note/20200908-My dir/";
        let output = tag_filter(&to_value(&input).unwrap(), &args).unwrap_or_default();
        assert_eq!("20200908-", output);
        //
        //
        // Test `prepend_dot`.
        let input = "md";
        let output = prepend_dot_filter(&to_value(&input).unwrap(), &args).unwrap_or_default();
        assert_eq!(".md", output);

        let input = "";
        let output = prepend_dot_filter(&to_value(&input).unwrap(), &args).unwrap_or_default();
        assert_eq!("", output);
        //
        //
        // Test file extension.
        let input =
            "/usr/local/WEB-SERVER-CONTENT/blog.getreu.net/projects/tp-note/20200908-My file.md";
        let output = ext_filter(&to_value(&input).unwrap(), &args).unwrap_or_default();
        assert_eq!("md", output);

        let input =
            "/usr/local/WEB-SERVER-CONTENT/blog.getreu.net/projects/tp-note/20200908-My dir/";
        let output = ext_filter(&to_value(&input).unwrap(), &args).unwrap_or_default();
        assert_eq!("", output);
    }

    #[test]
    fn test_disassemble_filename() {
        let expected = ("1_2_3-", "my_title--my_subtitle", "(1)", "md");
        let result = disassemble_filename(Path::new("/my/dir/1_2_3-my_title--my_subtitle(1).md"));
        assert_eq!(expected, result);
    }

    #[test]
    fn test_assemble_filename() {
        let expected = "1_2_3-my_file-1-.md".to_string();
        let result = assemble_filename("1_2_3-", "my_file", "-1-", "md");
        assert_eq!(expected, result);
    }

    #[test]
    fn test_remove_sort_tag_extension() {
        // Pattern found and removed.
        let expected = "my_stem";
        let result = remove_copy_counter("my_stem(78)");
        assert_eq!(expected, result);

        // Pattern found and removed.
        let expected = "my_stem-";
        let result = remove_copy_counter("my_stem-(78)");
        assert_eq!(expected, result);

        // Pattern found and removed.
        let expected = "my_stem_";
        let result = remove_copy_counter("my_stem_(78)");
        assert_eq!(expected, result);

        // Pattern not found.
        assert_eq!(expected, result);
        let expected = "my_stem_(78))";
        let result = remove_copy_counter("my_stem_(78))");
        assert_eq!(expected, result);

        // Pattern not found.
        let expected = "my_stem_)78)";
        let result = remove_copy_counter("my_stem_)78)");
        assert_eq!(expected, result);
    }

    #[test]
    fn test_append_sort_tag_extension() {
        let expected = "my_stem(987)";
        let result = append_copy_counter("my_stem", 987);
        assert_eq!(expected, result);

        let expected = "my_stem(987)";
        let result = append_copy_counter("my_stem_", 987);
        assert_eq!(expected, result);

        let expected = "my_stem(987)";
        let result = append_copy_counter("my_stem___", 987);
        assert_eq!(expected, result);

        let expected = "my_stem(987)";
        let result = append_copy_counter("my_stem-", 987);
        assert_eq!(expected, result);

        let expected = "my_stem(987)";
        let result = append_copy_counter("my_stem-_---", 987);
        assert_eq!(expected, result);
    }
    #[test]
    fn test_filename_exclude_copy_counter_eq() {
        let p1 = Path::new("/mypath/123-title(1).md");
        let p2 = Path::new("/mypath/123-title(3).md");
        let expected = true;
        let result = filename_exclude_copy_counter_eq(p1, p2);
        assert_eq!(expected, result);

        let p1 = Path::new("/mypath/123-title(1).md");
        let p2 = Path::new("/mypath/123-titlX(3).md");
        let expected = false;
        let result = filename_exclude_copy_counter_eq(p1, p2);
        assert_eq!(expected, result);
    }
}
