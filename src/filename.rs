//! Helper functions that deal with filenames.
use crate::config::CFG;
use crate::config::COPY_COUNTER_MAX;
use crate::config::NOTE_FILENAME_LEN_MAX;
use anyhow::{anyhow, Result};
use std::path::Path;
use std::path::PathBuf;

/// Shortens the stem of a filename so that
/// `file_stem.len()+file_extension.len() <= NOTE_FILENAME_LEN_MAX`.
/// If stem ends with a pattern similar to a copy counter,
/// append `-` to stem.
pub fn shorten_filename(mut fqfn: PathBuf) -> PathBuf {
    // Determine length of file-extension.
    let note_extension = fqfn
        .extension()
        .unwrap_or_default()
        .to_str()
        .unwrap_or_default();
    let note_extension_len = note_extension.len();

    // Limit length of file-stem.
    let mut note_stem = fqfn
        .file_stem()
        .unwrap_or_default()
        .to_str()
        .unwrap_or_default()
        .to_string();

    // Does this stem ending look similar to a copy counter?
    if note_stem.len() != remove_copy_counter(&note_stem).len() {
        // Add an additional separator.
        note_stem.push_str(&CFG.copy_counter_extra_separator);
    };

    // Limit the size of `fqfn`
    let mut note_stem_short = String::new();
    // `+1` reserves one byte for `.` before the extension.
    for i in (0..NOTE_FILENAME_LEN_MAX - (note_extension_len + 1)).rev() {
        if let Some(s) = note_stem.get(..=i) {
            note_stem_short = s.to_string();
            break;
        }
    }

    // Assemble.
    let mut note_filename = note_stem_short;
    note_filename.push('.');
    note_filename.push_str(note_extension);

    // Replace filename
    fqfn.set_file_name(note_filename);

    fqfn
}

/// When the path `p` exists on disk already, append some extension
/// with an incrementing counter to the sort-tag in `p` until
/// we find a free slot.
pub fn find_unused(p: PathBuf) -> Result<PathBuf, anyhow::Error> {
    if !p.exists() {
        return Ok(p);
    };

    let (sort_tag, _, stem, _copy_counter, ext) = disassemble(&p);

    let mut new_path = p.clone();

    // Try up to 99 sort-tag-extensions, then give up.
    for n in 1..COPY_COUNTER_MAX {
        let stem_copy_counter = append_copy_counter(&stem, n);
        let filename = assemble(&sort_tag, &stem_copy_counter, &"", &ext);
        new_path.set_file_name(filename);

        if !new_path.exists() {
            break;
        }
    }

    // This only happens, when we have 99 copies already. Should never happen.
    if new_path.exists() {
        return Err(anyhow!(format!(
            "can not find unused filename in directory:\n\
            \t{}\n\
            (only 99 copies are allowed).",
            p.parent()
                .unwrap_or(Path::new(""))
                .to_str()
                .unwrap_or_default()
        )));
    }

    Ok(new_path)
}

/// Check if 2 filenames are equal. Compare all parts, except the copy counter.
/// Consider 2 file identical even when they have a different copy counter.
pub fn exclude_copy_counter_eq(p1: &Path, p2: &Path) -> bool {
    let (sort_tag1, _, stem1, _, ext1) = disassemble(p1);
    let (sort_tag2, _, stem2, _, ext2) = disassemble(p2);
    sort_tag1 == sort_tag2 && stem1 == stem2 && ext1 == ext2
}

/// Helper function that decomposes a fully qualified path name
/// into (`sort_tag`, `stem_copy_counter_ext`, `stem`, `copy_counter`, `ext`).
pub fn disassemble(p: &Path) -> (&str, &str, &str, &str, &str) {
    let tag_stem_copy_counter_ext = p
        .file_name()
        .unwrap_or_default()
        .to_str()
        .unwrap_or_default();

    let stem_copy_counter_ext = tag_stem_copy_counter_ext
        .trim_start_matches(|c: char| c.is_numeric() || c == '-' || c == '_');

    let sort_tag = &tag_stem_copy_counter_ext
        [0..tag_stem_copy_counter_ext.len() - stem_copy_counter_ext.len()];

    let file_stem = p
        .file_stem()
        .unwrap_or_default()
        .to_str()
        .unwrap_or_default()
        .trim_start_matches(|c: char| c.is_numeric() || c == '-' || c == '_');

    // Trim `sort_tag`.
    let stem_copy_counter =
        file_stem.trim_start_matches(|c: char| c.is_numeric() || c == '-' || c == '_');

    let stem = remove_copy_counter(stem_copy_counter);

    let copy_counter = &stem_copy_counter[stem.len()..];

    let ext = p
        .extension()
        .unwrap_or_default()
        .to_str()
        .unwrap_or_default();
    (sort_tag, stem_copy_counter_ext, stem, copy_counter, ext)
}

/// Concatenates the 3 parameters.
pub fn assemble(sort_tag: &str, stem: &str, copy_counter: &str, extension: &str) -> String {
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

/// Helper function that trims the copy counter at the end of string.
/// If there is none, return the same.
#[inline]
pub fn remove_copy_counter(tag: &str) -> &str {
    // Strip closing brackets at the end.
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
    // And finally strip starting brackets.
    let tag3 = if let Some(t) = tag2.strip_suffix(&CFG.copy_counter_opening_brackets) {
        t
    } else {
        return tag;
    };

    tag3
}

/// Append a copy counter to the string.
#[inline]
pub fn append_copy_counter(stem: &str, n: usize) -> String {
    let mut stem = stem.to_string();
    stem.push_str(&CFG.copy_counter_opening_brackets);
    stem.push_str(&n.to_string());
    stem.push_str(&CFG.copy_counter_closing_brackets);
    stem
}

/// MarkupLanguage of the note content.
pub enum MarkupLanguage {
    Markdown,
    RestructuredText,
    Html,
    Txt,
    Unknown,
    None,
}

impl MarkupLanguage {
    /// Is `file_extension` listed in one of the known file extension
    /// lists?
    #[inline]
    pub fn new(file_extension: &str) -> Self {
        for e in &CFG.note_file_extensions_md {
            if e == file_extension {
                return MarkupLanguage::Markdown;
            }
        }
        for e in &CFG.note_file_extensions_rst {
            if e == file_extension {
                return MarkupLanguage::RestructuredText;
            }
        }
        for e in &CFG.note_file_extensions_html {
            if e == file_extension {
                return MarkupLanguage::Html;
            }
        }
        for e in &CFG.note_file_extensions_txt {
            if e == file_extension {
                return MarkupLanguage::Txt;
            }
        }
        for e in &CFG.note_file_extensions_no_viewer {
            if e == file_extension {
                return MarkupLanguage::Unknown;
            }
        }
        // If ever `extension_default` got forgotten in
        // one of the above lists, make sure that Tp-Note
        // recognizes its own files. Even without Markup
        // rendition.
        if file_extension == &CFG.extension_default {
            return MarkupLanguage::Txt;
        }
        MarkupLanguage::None
    }

    ///
    /// Is `extension` or the file extension of `path` listed in one of the known
    /// file extension lists?
    #[inline]
    pub fn from(extension: Option<&str>, path: &Path) -> Self {
        let file_extension = if let Some(ext) = extension {
            ext
        } else {
            path.extension()
                .unwrap_or_default()
                .to_str()
                .unwrap_or_default()
        };

        Self::new(file_extension)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_shorten_filename() {
        use std::ffi::OsString;
        use std::path::PathBuf;

        // Test short filename.
        let input = PathBuf::from("long directory name/abc.ext");
        let output = shorten_filename(input);
        assert_eq!(OsString::from("long directory name/abc.ext"), output);

        // Test long filename.
        let input = PathBuf::from("long directory name/long filename.ext");
        let output = shorten_filename(input);
        assert_eq!(OsString::from("long directory name/long f.ext"), output);

        // Test concatenation of extra `-` if it ends with a copy counter pattern.
        let input = "fn";
        // This makes the filename problematic
        let mut input = append_copy_counter(input, 1);
        let mut expected = input.clone();
        expected.push_str(&CFG.copy_counter_extra_separator);

        input.push_str(".ext");
        expected.push_str(".ext");

        let output = shorten_filename(PathBuf::from(input));
        assert_eq!(OsString::from(expected), output);
    }

    #[test]
    fn test_disassemble_filename() {
        let expected = (
            "1_2_3-",
            "my_title--my_subtitle(1).md",
            "my_title--my_subtitle",
            "(1)",
            "md",
        );
        let result = disassemble(Path::new("/my/dir/1_2_3-my_title--my_subtitle(1).md"));
        assert_eq!(expected, result);
    }

    #[test]
    fn test_assemble_filename() {
        let expected = "1_2_3-my_file-1-.md".to_string();
        let result = assemble("1_2_3-", "my_file", "-1-", "md");
        assert_eq!(expected, result);
    }

    #[test]
    fn test_remove_copy_counter() {
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
    }
    #[test]
    fn test_filename_exclude_copy_counter_eq() {
        let p1 = Path::new("/mypath/123-title(1).md");
        let p2 = Path::new("/mypath/123-title(3).md");
        let expected = true;
        let result = exclude_copy_counter_eq(p1, p2);
        assert_eq!(expected, result);

        let p1 = Path::new("/mypath/123-title(1).md");
        let p2 = Path::new("/mypath/123-titlX(3).md");
        let expected = false;
        let result = exclude_copy_counter_eq(p1, p2);
        assert_eq!(expected, result);
    }
}
