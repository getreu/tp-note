//! Helper funtions that deal with filenames.
extern crate sanitize_filename_reader_friendly;
use crate::config::CFG;
use anyhow::{anyhow, Result};
use std::path::Path;
use std::path::PathBuf;

/// When the path `p` exists on disk already, append some extension
/// with an incrementing counter to the sort-tag in `p` until
/// we find a free slot.
pub fn find_unused(p: PathBuf) -> Result<PathBuf, anyhow::Error> {
    if !p.exists() {
        return Ok(p);
    };

    let (sort_tag, stem, _copy_counter, ext) = disassemble(&p);

    let mut new_path = p.clone();

    // Try up to 99 sort-tag-extensions, then give up.
    for n in 1..99 {
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
    let (sort_tag1, stem1, _, ext1) = disassemble(p1);
    let (sort_tag2, stem2, _, ext2) = disassemble(p2);
    sort_tag1 == sort_tag2 && stem1 == stem2 && ext1 == ext2
}

/// Helper function that decomposes a fully qualified path name
/// into (parent_dir, sort_tag, file_stem_without_sort_tag, extension).
pub fn disassemble(p: &Path) -> (&str, &str, &str, &str) {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_disassemble_filename() {
        let expected = ("1_2_3-", "my_title--my_subtitle", "(1)", "md");
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
