//! Extends the built-in Tera filters.
extern crate sanitize_filename_reader_friendly;
use crate::config::Hyperlink;
use crate::filename::disassemble;
use lazy_static::lazy_static;
use sanitize_filename_reader_friendly::sanitize;
use std::collections::HashMap;
use std::hash::BuildHasher;
use std::ops::Deref;
use std::ops::DerefMut;
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
        tera.register_filter("remove", remove_filter);
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

    let (tag, _, _, _) = disassemble(Path::new(&p));

    Ok(to_value(&tag).unwrap())
}

/// A Tera filter that takes a path and extracts its file stem.
pub fn stem_filter<S: BuildHasher>(
    value: &Value,
    _args: &HashMap<String, Value, S>,
) -> TeraResult<Value> {
    let p = try_get_value!("stem", "value", String, value);

    let (_, stem, _, _) = disassemble(Path::new(&p));

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

    Ok(to_value(&ext).unwrap_or_default())
}

/// A Tera filter that takes a list of variables and remove
/// one.
pub fn remove_filter<S: BuildHasher>(
    value: &Value,
    args: &HashMap<String, Value, S>,
) -> TeraResult<Value> {
    let mut map = try_get_value!("remove", "value", tera::Map<String, tera::Value>, value);

    let var = match args.get("var") {
        Some(val) => try_get_value!("remove", "var", String, val),
        None => "".to_string(),
    };

    let _ = map.remove(var.trim_start_matches("fm_"));

    Ok(to_value(&map).unwrap_or_default())
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
}

/// Auto-dereference for convenient access to `tera::Content`.
impl Deref for ContextWrapper {
    type Target = tera::Context;

    fn deref(&self) -> &Self::Target {
        &self.ct
    }
}

/// Auto-dereference for convenient access to `tera::Content`.
impl DerefMut for ContextWrapper {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.ct
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
}
