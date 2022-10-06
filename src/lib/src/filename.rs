//! Helper functions that deal with filenames.
use crate::config::CFG2;
use crate::config::FILENAME_COPY_COUNTER_MAX;
use crate::config::FILENAME_DOTFILE_MARKER;
use crate::config::FILENAME_LEN_MAX;
use crate::error::FileError;
use std::path::Path;
use std::path::PathBuf;

/// Shortens the stem of a filename so that
/// `file_stem.len()+file_extension.len() <= FILENAME_LEN_MAX`.
/// If stem ends with a pattern similar to a copy counter,
/// append `-` to stem.
pub fn shorten_filename(mut file_path: PathBuf) -> PathBuf {
    // Determine length of file-extension.
    let note_extension = file_path
        .extension()
        .unwrap_or_default()
        .to_str()
        .unwrap_or_default();
    let note_extension_len = note_extension.len();

    // Limit length of file-stem.
    let mut note_stem = file_path
        .file_stem()
        .unwrap_or_default()
        .to_str()
        .unwrap_or_default()
        .to_string();

    // Does this stem ending look similar to a copy counter?
    if note_stem.len() != remove_copy_counter(&note_stem).len() {
        // Add an additional separator.
        let cfg2 = CFG2.read().unwrap();
        note_stem.push_str(&cfg2.filename.copy_counter_extra_separator);
    };

    // Limit the size of `file_path`
    let mut note_stem_short = String::new();
    // `+1` reserves one byte for `.` before the extension.
    for i in (0..FILENAME_LEN_MAX - (note_extension_len + 1)).rev() {
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
    file_path.set_file_name(note_filename);

    file_path
}

/// Check if a `path` is a "well formed" filename.
/// We consider it well formed,
/// * if `path` has no directory components, only
///   a filename, and
/// * if the filename is not empty, and
/// * if the filename is a dot file (len >1 and without whitespace), or
/// * if the filename has an extension.
/// A valid extension must not contain whitespace.
pub fn is_well_formed_filename(path: &Path) -> bool {
    let filename = path.file_name().unwrap_or_default();
    let ext = path
        .extension()
        .unwrap_or_default()
        .to_str()
        .unwrap_or_default();

    let is_filename = !filename.is_empty() && (filename == path);

    let filename = filename.to_str().unwrap_or_default();
    let is_dot_file = filename.starts_with(FILENAME_DOTFILE_MARKER)
            // We consider only dot files without whitespace.
            && (filename == filename.trim())
            && filename.split_whitespace().count() == 1;

    let has_extension = !ext.is_empty()
            // Make sure that there is no whitespace.
            && (ext == ext.trim())
            && ext.split_whitespace().count() == 1;

    is_filename && (is_dot_file || has_extension)
}

/// When the path `p` exists on disk already, append some extension
/// with an incrementing counter to the sort-tag in `p` until
/// we find a free slot.
pub fn find_unused(p: PathBuf) -> Result<PathBuf, FileError> {
    if !p.exists() {
        return Ok(p);
    };

    let (sort_tag, _, stem, _copy_counter, ext) = disassemble(&p);

    let mut new_path = p.clone();

    // Try up to 99 sort tag extensions, then give up.
    for n in 1..FILENAME_COPY_COUNTER_MAX {
        let stem_copy_counter = append_copy_counter(stem, n);
        let filename = assemble(sort_tag, &stem_copy_counter, "", ext);
        new_path.set_file_name(filename);

        if !new_path.exists() {
            break;
        }
    }

    // This only happens, when we have 99 copies already. Should never happen.
    if new_path.exists() {
        return Err(FileError::NoFreeFileName {
            directory: p.parent().unwrap_or_else(|| Path::new("")).to_path_buf(),
        });
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
    let cfg2 = CFG2.read().unwrap();

    let sort_tag_stem_copy_counter_ext = p
        .file_name()
        .unwrap_or_default()
        .to_str()
        .unwrap_or_default();

    let sort_tag_stem_copy_counter = p
        .file_stem()
        .unwrap_or_default()
        .to_str()
        .unwrap_or_default();

    let stem_copy_counter = sort_tag_stem_copy_counter
        .trim_start_matches(&cfg2.filename.sort_tag_chars.chars().collect::<Vec<char>>()[..]);

    let sort_tag =
        &sort_tag_stem_copy_counter[0..sort_tag_stem_copy_counter.len() - stem_copy_counter.len()];

    // Trim `sort_tag`.
    let stem_copy_counter_ext = if sort_tag_stem_copy_counter_ext.len() > sort_tag.len() {
        &sort_tag_stem_copy_counter_ext[sort_tag.len()..]
    } else {
        ""
    };

    // Trim `sort_tag`.
    let stem_copy_counter = if sort_tag_stem_copy_counter.len() > sort_tag.len() {
        &sort_tag_stem_copy_counter[sort_tag.len()..]
    } else {
        ""
    };

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
    let cfg2 = CFG2.read().unwrap();
    // Strip closing brackets at the end.
    let tag1 = if let Some(t) = tag.strip_suffix(&cfg2.filename.copy_counter_closing_brackets) {
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
    let tag3 = if let Some(t) = tag2.strip_suffix(&cfg2.filename.copy_counter_opening_brackets) {
        t
    } else {
        return tag;
    };

    tag3
}

/// Append a copy counter to the string.
#[inline]
pub fn append_copy_counter(stem: &str, n: usize) -> String {
    let cfg2 = CFG2.read().unwrap();
    let mut stem = stem.to_string();
    stem.push_str(&cfg2.filename.copy_counter_opening_brackets);
    stem.push_str(&n.to_string());
    stem.push_str(&cfg2.filename.copy_counter_closing_brackets);
    stem
}

/// The Markup language of the note content.
#[derive(Eq, PartialEq, Debug, Clone)]
pub enum MarkupLanguage {
    Markdown,
    RestructuredText,
    Html,
    Txt,
    Unknown,
    None,
}

impl MarkupLanguage {
    /// If `Self` is `None` return `rhs`, otherwise return `Self`.
    pub fn or(self, rhs: Self) -> Self {
        match self {
            MarkupLanguage::None => rhs,
            _ => self,
        }
    }
}

impl From<&str> for MarkupLanguage {
    /// Is `file_extension` listed in one of the known file extension
    /// lists?
    #[inline]
    fn from(file_extension: &str) -> Self {
        let cfg2 = CFG2.read().unwrap();

        for e in &cfg2.filename.extensions_md {
            if e == file_extension {
                return MarkupLanguage::Markdown;
            }
        }
        for e in &cfg2.filename.extensions_rst {
            if e == file_extension {
                return MarkupLanguage::RestructuredText;
            }
        }
        for e in &cfg2.filename.extensions_html {
            if e == file_extension {
                return MarkupLanguage::Html;
            }
        }
        for e in &cfg2.filename.extensions_txt {
            if e == file_extension {
                return MarkupLanguage::Txt;
            }
        }
        for e in &cfg2.filename.extensions_no_viewer {
            if e == file_extension {
                return MarkupLanguage::Unknown;
            }
        }
        // If ever `extension_default` got forgotten in
        // one of the above lists, make sure that Tp-Note
        // recognizes its own files. Even without Markup
        // rendition.
        if file_extension == cfg2.filename.extension_default {
            return MarkupLanguage::Txt;
        }
        MarkupLanguage::None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_shorten_filename() {
        use std::ffi::OsString;
        use std::path::PathBuf;
        let cfg2 = CFG2.read().unwrap();

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
        expected.push_str(&cfg2.filename.copy_counter_extra_separator);

        input.push_str(".ext");
        expected.push_str(".ext");

        let output = shorten_filename(PathBuf::from(input));
        assert_eq!(OsString::from(expected), output);
    }

    #[test]
    fn test_is_well_formed() {
        use std::path::Path;

        // Test long filename.
        assert_eq!(
            is_well_formed_filename(Path::new("long filename.ext")),
            true
        );

        // Test long file path.
        assert_eq!(
            is_well_formed_filename(Path::new("long directory name/long filename.ext")),
            false
        );

        // Test dot file.
        assert_eq!(is_well_formed_filename(Path::new(".dotfile")), true);

        // Test dot file with extension.
        assert_eq!(is_well_formed_filename(Path::new(".dotfile.ext")), true);

        // Test dot file with whitespace.
        assert_eq!(is_well_formed_filename(Path::new(".dot file")), false);

        // Test space in ext.
        assert_eq!(is_well_formed_filename(Path::new("filename.e xt")), false);

        // Test space in ext.
        assert_eq!(is_well_formed_filename(Path::new("filename. ext")), false);

        // Test space in ext.
        assert_eq!(is_well_formed_filename(Path::new("filename.ext ")), false);
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

        let expected = (
            "2021.04.12-",
            "my_title--my_subtitle(1).md",
            "my_title--my_subtitle",
            "(1)",
            "md",
        );
        let result = disassemble(Path::new("/my/dir/2021.04.12-my_title--my_subtitle(1).md"));
        assert_eq!(expected, result);

        let expected = (
            "2021 04 12 ",
            "my_title--my_subtitle(1).md",
            "my_title--my_subtitle",
            "(1)",
            "md",
        );
        let result = disassemble(Path::new("/my/dir/2021 04 12 my_title--my_subtitle(1).md"));
        assert_eq!(expected, result);

        let expected = ("2021 04 12 ", "", "", "", "");
        let result = disassemble(Path::new("/my/dir/2021 04 12 "));
        assert_eq!(expected, result);

        // This triggers the bug fixed with v1.14.3.
        let expected = ("2021 04 12 ", ".md", "", "", "md");
        let result = disassemble(Path::new("/my/dir/2021 04 12 .md"));
        assert_eq!(expected, result);

        let expected = ("2021 04 12 ", "(9).md", "", "(9)", "md");
        let result = disassemble(Path::new("/my/dir/2021 04 12 (9).md"));
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
