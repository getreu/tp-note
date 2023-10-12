//! Helper functions dealing with filenames.
use crate::config::FILENAME_COPY_COUNTER_MAX;
use crate::config::FILENAME_DOTFILE_MARKER;
use crate::config::FILENAME_LEN_MAX;
use crate::config::LIB_CFG;
use crate::error::FileError;
use crate::markup_language::MarkupLanguage;
use std::mem::swap;
use std::path::Path;
use std::path::PathBuf;

pub(crate) const FILENAME_EXTENSION_SEPARATOR_DOT: char = '.';

/// Extents `PathBuf` with methods dealing with paths to Tp-Note files.
pub trait NotePathBuf {
    /// Concatenates the `sort_tag`, `stem`, `copy_counter`, `.` and
    /// `extension`.
    /// This functions inserts all potentially necessary separators and
    /// extra separators.
    fn from_disassembled(
        sort_tag: &str,
        stem: &str,
        copy_counter: Option<usize>,
        extension: &str,
    ) -> Self;
    /// Append/increment a copy counter.
    /// When the path `p` exists on disk already, append some extension
    /// with an incrementing counter to the sort-tag in `p` until
    /// we find a free unused filename.
    /// ```rust
    /// use std::env::temp_dir;
    /// use std::fs;
    /// use tpnote_lib::filename::NotePathBuf;
    ///
    /// // Prepare test: create existing note file.
    /// let raw = "some content";
    /// let mut notefile = temp_dir().join("20221101-My day--Note.md");
    /// fs::write(&notefile, raw.as_bytes()).unwrap();
    /// let expected = temp_dir().join("20221101-My day--Note(1).md");
    /// let _ = fs::remove_file(&expected);
    ///
    /// // Start test
    /// notefile.set_next_unused();
    /// assert_eq!(notefile, expected);
    /// ```
    ///
    /// When the filename is not used, keep it.
    /// ```rust
    /// use std::env::temp_dir;
    /// use std::fs;
    /// use tpnote_lib::filename::NotePathBuf;
    ///
    /// // Prepare test: make sure that there is no note file.
    /// let mut notefile = temp_dir().join("20221101-My day--Note.md");
    /// let _ = fs::remove_file(&notefile);
    /// // The name should not change.
    /// let expected = notefile.clone();
    ///
    /// // Start test
    /// notefile.set_next_unused();
    /// assert_eq!(notefile, expected);
    /// ```

    fn set_next_unused(&mut self) -> Result<(), FileError>;

    /// Shortens the stem of a filename so that
    /// `filename.len() <= FILENAME_LEN_MAX`.
    /// This method assumes, that the file stem does not contain a copy
    /// counter. If stem ends with a pattern similar to a copy counter,
    /// it appends `-` to stem (cf. unit test in the source code).
    ///
    /// ```rust
    /// use std::ffi::OsString;
    /// use std::path::PathBuf;
    /// use tpnote_lib::filename::NotePathBuf;
    /// use tpnote_lib::config::FILENAME_LEN_MAX;
    ///
    /// // Test short filename.
    /// let mut input = PathBuf::from("short filename.md");
    /// input.shorten_filename();
    /// let output = input;
    /// assert_eq!(OsString::from("short filename.md"),
    ///            output.into_os_string());
    ///
    /// // Test too long filename.
    /// let mut input = String::from("some/path/");
    /// for _ in 0..(FILENAME_LEN_MAX - "long fi.ext".len()-1) {
    ///     input.push('x');
    /// }
    /// let mut expected = input.clone();
    /// input.push_str("long filename to be cut.ext");
    /// let mut input = PathBuf::from(input);
    /// expected.push_str("long fi.ext");
    ///
    /// input.shorten_filename();
    /// let output = PathBuf::from(input);
    /// assert_eq!(PathBuf::from(expected), output);
    /// ```
    fn shorten_filename(&mut self);
}

impl NotePathBuf for PathBuf {
    #[inline]

    fn from_disassembled(
        sort_tag: &str,
        stem: &str,
        copy_counter: Option<usize>,
        extension: &str,
    ) -> Self {
        // Assemble path.
        let mut filename = String::new();

        // Add potential sort-tag and separators.
        let lib_cfg = LIB_CFG.read_recursive();
        if !sort_tag.is_empty() {
            filename.push_str(sort_tag);
            filename.push_str(&lib_cfg.filename.sort_tag_separator);
        }
        // Does the beginning of `stem` look like a sort-tag?
        // Make sure, that the path can not be misinterpreted, even if a
        // `sort_tag_separator` would follow.
        let mut test_path = String::from(stem);
        test_path.push_str(&lib_cfg.filename.sort_tag_separator);
        // Do we need an `extra_separator`?
        if stem.is_empty() || !Path::split_sort_tag(&test_path).0.is_empty() {
            filename.push(lib_cfg.filename.sort_tag_extra_separator);
        }

        filename.push_str(stem);

        if let Some(cc) = copy_counter {
            // Is `copy_counter_extra_separator` necessary?
            // Does this stem ending look similar to a copy counter?
            if Path::split_copy_counter(stem).1.is_some() {
                // Add an additional separator.
                filename.push_str(&lib_cfg.filename.copy_counter_extra_separator);
            };

            filename.push_str(&lib_cfg.filename.copy_counter_opening_brackets);
            filename.push_str(&cc.to_string());
            filename.push_str(&lib_cfg.filename.copy_counter_closing_brackets);
        }

        if !extension.is_empty() {
            filename.push(FILENAME_EXTENSION_SEPARATOR_DOT);
            filename.push_str(extension);
        };
        PathBuf::from(filename)
    }

    fn set_next_unused(&mut self) -> Result<(), FileError> {
        if !&self.exists() {
            return Ok(());
        };

        let (sort_tag, _, stem, _copy_counter, ext) = self.disassemble();

        let mut new_path = self.clone();

        // Try up to 99 sort tag extensions, then give up.
        for copy_counter in 1..FILENAME_COPY_COUNTER_MAX {
            let filename = Self::from_disassembled(sort_tag, stem, Some(copy_counter), ext);
            new_path.set_file_name(filename);

            if !new_path.exists() {
                break;
            }
        }

        // This only happens, when we have 99 copies already. Should never happen.
        if new_path.exists() {
            return Err(FileError::NoFreeFileName {
                directory: self.parent().unwrap_or_else(|| Path::new("")).to_path_buf(),
            });
        }
        swap(self, &mut new_path);
        Ok(())
    }

    fn shorten_filename(&mut self) {
        // Determine length of file-extension.
        let stem = self
            .file_stem()
            .unwrap_or_default()
            .to_str()
            .unwrap_or_default();
        let ext = self
            .extension()
            .unwrap_or_default()
            .to_str()
            .unwrap_or_default();
        let ext_len = ext.len();

        // Limit the size of the filename.
        let mut stem_short = String::new();
        // `+1` reserves one byte for `.` before the extension.
        // `+1` reserves one byte for `-` a potential copy counter extra
        // separator.
        for i in (0..FILENAME_LEN_MAX - (ext_len + 2)).rev() {
            if let Some(s) = stem.get(..=i) {
                stem_short = s.to_string();
                break;
            }
        }

        // Does this ending look like a copy counter?
        if Path::split_copy_counter(&stem_short).1.is_some() {
            let lib_cfg = LIB_CFG.read_recursive();
            stem_short.push_str(&lib_cfg.filename.copy_counter_extra_separator);
        }

        // Assemble.
        let mut note_filename = stem_short;
        if !ext.is_empty() {
            note_filename.push(FILENAME_DOTFILE_MARKER);
            note_filename.push_str(ext);
        }
        // Replace filename`
        self.set_file_name(note_filename);
    }
}

/// Trait that interprets the implenting type as filename extension.
pub(crate) trait Extension {
    /// Returns `True` if `self` is equal to one of the Tp-Note extensions
    /// registered in the configuration file `filename.extensions` table.
    fn is_tpnote_ext(&self) -> bool;
    /// Returns `True` is the path in `self` ends with an extension, that
    /// registered as Tp-Note extension in `filename.extensions`.
    fn has_tpnote_ext(&self) -> bool;
}

impl Extension for str {
    fn is_tpnote_ext(&self) -> bool {
        MarkupLanguage::from(self).is_some()
    }

    fn has_tpnote_ext(&self) -> bool {
        MarkupLanguage::from(Path::new(self)).is_some()
    }
}

/// Some private helper functions related to note filenames.
pub(crate) trait NotePathPrivate {
    /// Helper function: Greedliy match sort tags and return it as
    /// a subslice as first tuple and the rest as second tuple. If
    /// `filename.sort_tag_separator` is defined and it can be detected after
    /// the matched subslice, skip it and return the rest of the string as
    /// second tuple. If `filename.sort_tag_separator` is defined, but the
    /// separator can not be found, discard the matched sort tag and return `("",
    /// sort_tag_stem_copy_counter)`.
    /// Techical note: A sort tag is identified with the following regular
    /// expression in SED syntax: `[0-9_- .\t]*-`.
    /// The expression can be customized as follows:
    /// * `[0-9_- .\t]` with `filename.sort_tag_chars`,
    /// * `-` with `filename.sort_tag_separator` and
    /// * `'` with `filename.sort_tag_extra_separator`.
    fn split_sort_tag(sort_tag_stem_copy_counter_ext: &str) -> (&str, &str) {
        let lib_cfg = LIB_CFG.read_recursive();

        let mut sort_tag = &sort_tag_stem_copy_counter_ext[..sort_tag_stem_copy_counter_ext
            .chars()
            .take_while(|&c| lib_cfg.filename.sort_tag_chars.contains([c]))
            .count()];

        let mut stem_copy_counter_ext;
        if lib_cfg.filename.sort_tag_separator.is_empty() {
            // `sort_tag` is correct.
            stem_copy_counter_ext = &sort_tag_stem_copy_counter_ext[sort_tag.len()..];
        } else {
            // Take `sort_tag_separator` into account.
            if let Some(i) = sort_tag.rfind(&lib_cfg.filename.sort_tag_separator) {
                sort_tag = &sort_tag[..i];
                stem_copy_counter_ext = &sort_tag_stem_copy_counter_ext
                    [i + lib_cfg.filename.sort_tag_separator.len()..];
            } else {
                sort_tag = "";
                stem_copy_counter_ext = sort_tag_stem_copy_counter_ext;
            }
        }

        // Remove `sort_tag_extra_separator` if it is at the first position
        // followed by a `sort_tag_char` at the second position.
        let mut chars = stem_copy_counter_ext.chars();
        if chars
            .next()
            .is_some_and(|c| c == lib_cfg.filename.sort_tag_extra_separator)
            && chars
                .next()
                .is_some_and(|c| lib_cfg.filename.sort_tag_chars.find(c).is_some())
        {
            stem_copy_counter_ext = stem_copy_counter_ext
                .strip_prefix(lib_cfg.filename.sort_tag_extra_separator)
                .unwrap();
        }

        (sort_tag, stem_copy_counter_ext)
    }

    /// Helper function that trims the copy counter at the end of string,
    /// returns the result and the copy counter.
    /// This function removes all brackets and a potiential extra separator.
    #[inline]
    fn split_copy_counter(file_stem: &str) -> (&str, Option<usize>) {
        let lib_cfg = LIB_CFG.read_recursive();
        // Strip closing brackets at the end.
        let tag1 = if let Some(t) =
            file_stem.strip_suffix(&lib_cfg.filename.copy_counter_closing_brackets)
        {
            t
        } else {
            return (file_stem, None);
        };
        // Now strip numbers.
        let tag2 = tag1.trim_end_matches(|c: char| c.is_numeric());
        let copy_counter: Option<usize> = if tag2.len() < tag1.len() {
            tag1[tag2.len()..].parse().ok()
        } else {
            return (file_stem, None);
        };
        // And finally strip starting bracket.
        let tag3 =
            if let Some(t) = tag2.strip_suffix(&lib_cfg.filename.copy_counter_opening_brackets) {
                t
            } else {
                return (file_stem, None);
            };
        // This is optional
        if let Some(t) = tag3.strip_suffix(&lib_cfg.filename.copy_counter_extra_separator) {
            (t, copy_counter)
        } else {
            (tag3, copy_counter)
        }
    }
}

impl NotePathPrivate for Path {}

/// Extents `Path` with methods dealing with paths to Tp-Note files.
pub trait NotePath {
    /// Helper function that decomposes a fully qualified path name
    /// into (`sort_tag`, `stem_copy_counter_ext`, `stem`, `copy_counter`, `ext`).
    /// All sort-tag seprators and copy-counter separators/brackets are removed.
    fn disassemble(&self) -> (&str, &str, &str, Option<usize>, &str);
    /// Compares with another `Path` to a Tp-Note file. They are considered equal
    /// even when the copy counter is different.
    fn exclude_copy_counter_eq(&self, p2: &Path) -> bool;
    /// Check if the filename of `Path` contains only
    /// `lib_cfg.filename.sort_tag_chars` and return it.
    fn filename_contains_only_sort_tag_chars(&self) -> Option<&str>;
    /// Check if a `Path` points to a file with a "wellformed" filename.
    fn has_wellformed_filename(&self) -> bool;
    /// Compare to all file extensions Tp-Note can open.
    fn has_tpnote_ext(&self) -> bool;
}

impl NotePath for Path {
    fn disassemble(&self) -> (&str, &str, &str, Option<usize>, &str) {
        let sort_tag_stem_copy_counter_ext = self
            .file_name()
            .unwrap_or_default()
            .to_str()
            .unwrap_or_default();

        let (sort_tag, stem_copy_counter_ext) =
            Self::split_sort_tag(sort_tag_stem_copy_counter_ext);

        let stem_copy_counter = Path::new(stem_copy_counter_ext)
            .file_stem()
            .unwrap_or_default()
            .to_str()
            .unwrap_or_default(); // Trim `sort_tag`.

        let ext = if stem_copy_counter == stem_copy_counter_ext {
            ""
        } else {
            // `Path::new()` is a cost free conversion.
            Path::new(stem_copy_counter_ext)
                .extension()
                .unwrap_or_default()
                .to_str()
                .unwrap_or_default()
        };

        let (stem, copy_counter) = Self::split_copy_counter(stem_copy_counter);

        (sort_tag, stem_copy_counter_ext, stem, copy_counter, ext)
    }

    /// Check if 2 filenames are equal. Compare all parts, except the copy counter.
    /// Consider 2 file identical even when they have a different copy counter.
    fn exclude_copy_counter_eq(&self, p2: &Path) -> bool {
        let (sort_tag1, _, stem1, _, ext1) = self.disassemble();
        let (sort_tag2, _, stem2, _, ext2) = p2.disassemble();
        sort_tag1 == sort_tag2 && stem1 == stem2 && ext1 == ext2
    }

    /// Check if a the filename of `path` contains only sort tag chars. If
    /// yes, return it.
    ///
    /// ```rust
    /// use std::path::Path;
    /// use tpnote_lib::filename::NotePath;
    ///
    /// let f = Path::new("20230821-");
    /// assert_eq!(f.filename_contains_only_sort_tag_chars(), Some("20230821-"));
    ///
    /// let f = Path::new("20230821");
    /// assert_eq!(f.filename_contains_only_sort_tag_chars(), Some("20230821"));
    ///
    /// let f = Path::new("2023");
    /// assert_eq!(f.filename_contains_only_sort_tag_chars(), Some("2023"));
    ///
    /// let f = Path::new("20230821-A");
    /// assert_eq!(f.filename_contains_only_sort_tag_chars(), None);
    /// ```
    fn filename_contains_only_sort_tag_chars(&self) -> Option<&str> {
        let filename = self
            .file_name()
            .unwrap_or_default()
            .to_str()
            .unwrap_or_default();

        let lib_cfg = LIB_CFG.read_recursive();

        if !filename.is_empty()
            && filename
                .chars()
                .all(|c| lib_cfg.filename.sort_tag_chars.contains([c]))
        {
            Some(filename)
        } else {
            None
        }
    }

    /// Check if a `path` points to a file with a
    /// "well formed" filename.
    /// We consider it well formed,
    /// * if the filename is not empty, and
    ///   * if the filename is a dot file (len >1 and without whitespace), or
    ///   * if the filename has an extension.
    /// A valid extension must not contain whitespace.
    ///
    /// ```rust
    /// use std::path::Path;
    /// use tpnote_lib::filename::NotePath;
    ///
    /// let f = Path::new("tpnote.toml");
    /// assert!(f.has_wellformed_filename());
    ///
    /// let f = Path::new("dir/tpnote.toml");
    /// assert!(f.has_wellformed_filename());
    ///
    /// let f = Path::new("tpnote.to ml");
    /// assert!(!f.has_wellformed_filename());
    /// ```
    fn has_wellformed_filename(&self) -> bool {
        let filename = &self.file_name().unwrap_or_default();
        let ext = self
            .extension()
            .unwrap_or_default()
            .to_str()
            .unwrap_or_default();

        let is_filename = !filename.is_empty();

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

    /// Returns `True` if the path in `self` ends with an extension, that Tp-
    /// Note considers as it's own file. To do so, the extension is compared
    /// to all items in the registered `filename.extensions` table in the
    /// configuration file.
    fn has_tpnote_ext(&self) -> bool {
        MarkupLanguage::from(self).is_some()
    }
}

#[cfg(test)]
mod tests {
    use super::NotePath;
    use super::NotePathBuf;
    use super::NotePathPrivate;
    use crate::config::FILENAME_LEN_MAX;
    use crate::filename::Extension;
    use std::path::Path;
    use std::path::PathBuf;

    #[test]
    fn test_from_disassembled() {
        let expected = PathBuf::from("my_file.md");
        let result = PathBuf::from_disassembled("", "my_file", None, "md");
        assert_eq!(expected, result);

        let expected = PathBuf::from("1_2_3-my_file(1).md");
        let result = PathBuf::from_disassembled("1_2_3", "my_file", Some(1), "md");
        assert_eq!(expected, result);

        let expected = PathBuf::from("1_2_3-123 My_file(1).md");
        let result = PathBuf::from_disassembled("1_2_3", "123 My_file", Some(1), "md");
        assert_eq!(expected, result);

        let expected = PathBuf::from("1_2_3-'123-my_file(1).md");
        let result = PathBuf::from_disassembled("1_2_3", "123-my_file", Some(1), "md");
        assert_eq!(expected, result);

        let expected = PathBuf::from("'123-my_file(1).md");
        let result = PathBuf::from_disassembled("", "123-my_file", Some(1), "md");
        assert_eq!(expected, result);

        let res = PathBuf::from_disassembled("1234", "title--subtitle", Some(9), "md");
        assert_eq!(res, Path::new("1234-title--subtitle(9).md"));

        let res = PathBuf::from_disassembled("1234", "5678", Some(9), "md");
        assert_eq!(res, Path::new("1234-'5678(9).md"));

        let res = PathBuf::from_disassembled("1234", "5678--subtitle", Some(9), "md");
        assert_eq!(res, Path::new("1234-'5678--subtitle(9).md"));

        let res = PathBuf::from_disassembled("1234", "", None, "md");
        assert_eq!(res, Path::new("1234-'.md"));

        // This is a special case, that can not be disassembled properly.
        let res = PathBuf::from_disassembled("1234", "'5678--subtitle", Some(9), "md");
        assert_eq!(res, Path::new("1234-'5678--subtitle(9).md"));

        let res = PathBuf::from_disassembled("", "-", Some(9), "md");
        assert_eq!(res, Path::new("'-(9).md"));

        let res = PathBuf::from_disassembled("", "(1)", Some(9), "md");
        assert_eq!(res, Path::new("(1)-(9).md"));

        // This is a special case, that can not be disassembled properly.
        let res = PathBuf::from_disassembled("", "(1)-", Some(9), "md");
        assert_eq!(res, Path::new("(1)-(9).md"));
    }

    #[test]
    fn test_shorten_filename() {
        use std::ffi::OsString;
        use std::path::PathBuf;

        // Test a short filename with a problematic file stem ending looking
        // like a copy counter pattern. Therefor the method appends `-`.
        let mut input = PathBuf::from("fn(1).md");
        let expected = PathBuf::from("fn(1)-.md");
        // As this filename is too short, `shorten_filename()` should not change
        // anything.
        input.shorten_filename();
        let output = input;
        assert_eq!(OsString::from(expected), output);

        //
        // Test if assembled correctly.
        let mut input = PathBuf::from("20221030-some.pdf--Note.md");
        let expected = input.clone();
        input.shorten_filename();
        let output = input;
        assert_eq!(OsString::from(expected), output);

        //
        // Test long filename.
        let mut input = std::iter::repeat("X")
            .take(FILENAME_LEN_MAX + 10)
            .collect::<String>();
        input.push_str(".ext");

        let mut expected = std::iter::repeat("X")
            .take(FILENAME_LEN_MAX - ".ext".len() - 1)
            .collect::<String>();
        expected.push_str(".ext");

        let mut input = PathBuf::from(input);
        input.shorten_filename();
        let output = input;
        assert_eq!(OsString::from(expected), output);
    }

    #[test]
    fn test_set_next_unused() {
        use std::env::temp_dir;
        use std::fs;

        let raw = "This simulates a non tp-note file";
        let mut notefile = temp_dir().join("20221030-some.pdf--Note.md");
        fs::write(&notefile, raw.as_bytes()).unwrap();

        notefile.set_next_unused().unwrap();
        let expected = temp_dir().join("20221030-some.pdf--Note(1).md");
        assert_eq!(notefile, expected);
        let _ = fs::remove_file(notefile);
    }

    #[test]
    fn test_has_wellformed() {
        use std::path::Path;

        // Test long filename.
        assert!(&Path::new("long filename.ext").has_wellformed_filename());

        // Test long file path, this fails.
        assert!(&Path::new("long directory name/long filename.ext").has_wellformed_filename());

        // Test dot file
        assert!(&Path::new(".dotfile").has_wellformed_filename());

        // Test dot file with extension.
        assert!(&Path::new(".dotfile.ext").has_wellformed_filename());

        // Test dot file with whitespace, this fails.
        assert!(!&Path::new(".dot file").has_wellformed_filename());

        // Test space in ext, this fails.
        assert!(!&Path::new("filename.e xt").has_wellformed_filename());

        // Test space in ext, this fails.
        assert!(!&Path::new("filename. ext").has_wellformed_filename());

        // Test space in ext, this fails.
        assert!(!&Path::new("filename.ext ").has_wellformed_filename());

        // Test path.
        assert!(&Path::new("/path/to/filename.ext").has_wellformed_filename());
    }

    #[test]
    fn test_disassemble_filename() {
        let expected = (
            "1_2_3",
            "my_title--my_subtitle(1).md",
            "my_title--my_subtitle",
            Some(1),
            "md",
        );
        let p = Path::new("/my/dir/1_2_3-my_title--my_subtitle(1).md");
        let result = p.disassemble();
        assert_eq!(expected, result);

        let expected = (
            "1_2_3",
            "my_title--my_subtitle(1)-(9).md",
            "my_title--my_subtitle(1)",
            Some(9),
            "md",
        );
        let p = Path::new("/my/dir/1_2_3-my_title--my_subtitle(1)-(9).md");
        let result = p.disassemble();
        assert_eq!(expected, result);

        let expected = (
            "2021.04.12",
            "my_title--my_subtitle(1).md",
            "my_title--my_subtitle",
            Some(1),
            "md",
        );
        let p = Path::new("/my/dir/2021.04.12-my_title--my_subtitle(1).md");
        let result = p.disassemble();
        assert_eq!(expected, result);

        let expected = (
            "",
            "2021 04 12 my_title--my_subtitle(1).md",
            "2021 04 12 my_title--my_subtitle",
            Some(1),
            "md",
        );
        let p = Path::new("/my/dir/2021 04 12 my_title--my_subtitle(1).md");
        let result = p.disassemble();
        assert_eq!(expected, result);

        let expected = ("2021 04 12", "", "", None, "");
        let p = Path::new("/my/dir/2021 04 12-");
        let result = p.disassemble();
        assert_eq!(expected, result);

        // This triggers the bug fixed with v1.14.3.
        let expected = ("2021 04 12", ".dotfile", ".dotfile", None, "");
        let p = Path::new("/my/dir/2021 04 12-'.dotfile");
        let result = p.disassemble();
        assert_eq!(expected, result);

        let expected = ("2021 04 12", "(9).md", "", Some(9), "md");
        let p = Path::new("/my/dir/2021 04 12-(9).md");
        let result = p.disassemble();
        assert_eq!(expected, result);

        let expected = (
            "20221030",
            "some.pdf--Note.md",
            "some.pdf--Note",
            None,
            "md",
        );
        let p = Path::new("/my/dir/20221030-some.pdf--Note.md");
        let result = p.disassemble();
        assert_eq!(expected, result);

        let expected = (
            "1_2_3",
            "my_title--my_subtitle(1).md",
            "my_title--my_subtitle",
            Some(1),
            "md",
        );
        let p = Path::new("/my/dir/1_2_3-my_title--my_subtitle(1).md");
        let result = p.disassemble();
        assert_eq!(expected, result);

        let expected = (
            "1_2_3",
            "123 my_title--my_subtitle(1).md",
            "123 my_title--my_subtitle",
            Some(1),
            "md",
        );
        let p = Path::new("/my/dir/1_2_3-123 my_title--my_subtitle(1).md");
        let result = p.disassemble();
        assert_eq!(expected, result);

        let expected = (
            "1_2_3-123",
            "my_title--my_subtitle(1).md",
            "my_title--my_subtitle",
            Some(1),
            "md",
        );
        let p = Path::new("/my/dir/1_2_3-123-my_title--my_subtitle(1).md");
        let result = p.disassemble();
        assert_eq!(expected, result);

        let expected = (
            "1_2_3",
            "123-my_title--my_subtitle(1).md",
            "123-my_title--my_subtitle",
            Some(1),
            "md",
        );
        let p = Path::new("/my/dir/1_2_3-'123-my_title--my_subtitle(1).md");
        let result = p.disassemble();
        assert_eq!(expected, result);

        let expected = (
            "1_2_3",
            "123 my_title--my_subtitle(1).md",
            "123 my_title--my_subtitle",
            Some(1),
            "md",
        );
        let p = Path::new("/my/dir/1_2_3-123 my_title--my_subtitle(1).md");
        let result = p.disassemble();
        assert_eq!(expected, result);

        let expected = (
            "1_2_3",
            "'my'_title--my_subtitle(1).md",
            "'my'_title--my_subtitle",
            Some(1),
            "md",
        );
        let p = Path::new("/my/dir/1_2_3-'my'_title--my_subtitle(1).md");
        let result = p.disassemble();
        assert_eq!(expected, result);
    }

    #[test]
    fn test_trim_copy_counter() {
        // Pattern found and removed.
        let expected = ("my_stem", Some(78));
        let result = Path::split_copy_counter("my_stem(78)");
        assert_eq!(expected, result);

        // Pattern found and removed.
        let expected = ("my_stem", Some(78));
        let result = Path::split_copy_counter("my_stem-(78)");
        assert_eq!(expected, result);

        // Pattern found and removed.
        let expected = ("my_stem_", Some(78));
        let result = Path::split_copy_counter("my_stem_(78)");
        assert_eq!(expected, result);

        // Pattern not found.
        assert_eq!(expected, result);
        let expected = ("my_stem_(78))", None);
        let result = Path::split_copy_counter("my_stem_(78))");
        assert_eq!(expected, result);

        // Pattern not found.
        let expected = ("my_stem_)78)", None);
        let result = Path::split_copy_counter("my_stem_)78)");
        assert_eq!(expected, result);
    }

    #[test]
    fn test_filename_exclude_copy_counter_eq() {
        let p1 = PathBuf::from("/mypath/123-title(1).md");
        let p2 = PathBuf::from("/mypath/123-title(3).md");
        let expected = true;
        let result = Path::exclude_copy_counter_eq(&p1, &p2);
        assert_eq!(expected, result);

        let p1 = PathBuf::from("/mypath/123-title(1).md");
        let p2 = PathBuf::from("/mypath/123-titlX(3).md");
        let expected = false;
        let result = Path::exclude_copy_counter_eq(&p1, &p2);
        assert_eq!(expected, result);
    }

    #[test]
    fn test_split_sort_tag() {
        let expected = ("123", "Rest");
        let result = Path::split_sort_tag("123-Rest");
        assert_eq!(expected, result);

        let expected = ("123", "Rest");
        let result = Path::split_sort_tag("123-Rest");
        assert_eq!(expected, result);

        let expected = ("123-", "Rest");
        let result = Path::split_sort_tag("123--Rest");
        assert_eq!(expected, result);
    }

    #[test]
    fn test_note_path_has_tpnote_ext() {
        //
        let path = Path::new("/dir/file.md");
        assert!(<Path as NotePath>::has_tpnote_ext(path));

        //
        let path = Path::new("/dir/file.abc");
        assert!(!<Path as NotePath>::has_tpnote_ext(path));

        // This goes wrong because a file path or at least a filename is
        // expected here.
        let path = Path::new("md");
        assert!(!<Path as NotePath>::has_tpnote_ext(path));
    }

    #[test]
    fn test_extension_has_tpnote_ext_is_tpnote_ext() {
        //
        let path = "/dir/file.md";
        assert!(<str as Extension>::has_tpnote_ext(path));

        //
        let path = "/dir/file.abc";
        assert!(!<str as Extension>::has_tpnote_ext(path));

        //
        let ext = "md";
        assert!(<str as Extension>::is_tpnote_ext(ext));

        // This goes wrong because only `md` is expected here.
        let ext = "/dir/file.md";
        assert!(!<str as Extension>::is_tpnote_ext(ext));
    }
}
