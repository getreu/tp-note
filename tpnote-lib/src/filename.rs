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

/// Extents `PathBuf` with methods dealing with paths to Tp-Note files.
pub trait NotePathBuf {
    /// Concatenates the `sort_tag`, `stem`, `copy_counter`, `.` and `extension`.
    fn from_disassembled(sort_tag: &str, stem: &str, copy_counter: &str, extension: &str) -> Self;
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
    /// If stem ends with a pattern similar to a copy counter,
    /// append `-` to stem (cf. unit test in the source code).
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
    /// for _ in 0..(FILENAME_LEN_MAX - "long fil.ext".len()) {
    ///     input.push('x');
    /// }
    /// let mut expected = input.clone();
    /// input.push_str("long filename to be cut.ext");
    /// let mut input = PathBuf::from(input);
    /// expected.push_str("long fil.ext");
    ///
    /// input.shorten_filename();
    /// let output = PathBuf::from(input);
    /// assert_eq!(PathBuf::from(expected), output);
    /// ```
    fn shorten_filename(&mut self);
}

impl NotePathBuf for PathBuf {
    #[inline]

    fn from_disassembled(sort_tag: &str, stem: &str, copy_counter: &str, extension: &str) -> Self {
        // Assemble path.
        let mut filename = sort_tag.to_string();
        filename.push_str(stem);
        filename.push_str(copy_counter);
        if !extension.is_empty() {
            filename.push('.');
            filename.push_str(extension);
        };
        PathBuf::from(filename)
    }

    fn set_next_unused(&mut self) -> Result<(), FileError> {
        #[inline]
        fn append_copy_counter(stem: &str, n: usize) -> String {
            let lib_cfg = LIB_CFG.read_recursive();
            let mut stem = stem.to_string();
            stem.push_str(&lib_cfg.filename.copy_counter_opening_brackets);
            stem.push_str(&n.to_string());
            stem.push_str(&lib_cfg.filename.copy_counter_closing_brackets);
            stem
        }

        if !&self.exists() {
            return Ok(());
        };

        let (sort_tag, _, stem, _copy_counter, ext) = &self.disassemble();

        let mut new_path = self.clone();

        // Try up to 99 sort tag extensions, then give up.
        for n in 1..FILENAME_COPY_COUNTER_MAX {
            let stem_copy_counter = append_copy_counter(stem, n);
            let filename = Self::from_disassembled(sort_tag, &stem_copy_counter, "", ext);
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
        let note_extension = self
            .extension()
            .unwrap_or_default()
            .to_str()
            .unwrap_or_default();
        let note_extension_len = note_extension.len();

        // Limit length of file-stem.
        let mut note_stem = self
            .file_stem()
            .unwrap_or_default()
            .to_str()
            .unwrap_or_default()
            .to_string();

        // Does this stem ending look similar to a copy counter?
        if note_stem.len() != Path::trim_copy_counter(&note_stem).len() {
            // Add an additional separator.
            let lib_cfg = LIB_CFG.read_recursive();
            note_stem.push_str(&lib_cfg.filename.copy_counter_extra_separator);
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

        // Replace filename`
        self.set_file_name(note_filename);
    }
}

/// Extents `Path` with methods dealing with paths to Tp-Note files.
pub trait NotePath {
    /// Helper function that decomposes a fully qualified path name
    /// into (`sort_tag`, `stem_copy_counter_ext`, `stem`, `copy_counter`, `ext`).
    fn disassemble(&self) -> (&str, &str, &str, &str, &str);
    /// Compares with another `Path` to a Tp-Note file. They are considered equal
    /// even when the copy counter is different.
    fn exclude_copy_counter_eq(&self, p2: &Path) -> bool;
    /// Check if a `Path` points to a file with a "wellformed" filename.
    fn has_wellformed_filename(&self) -> bool;
    /// Helper function: Removes the copy counter from the file stem.
    fn trim_copy_counter(tag: &str) -> &str;
    /// Compare to all file extensions Tp-Note can open.
    fn has_tpnote_extension(&self) -> bool;
}

impl NotePath for Path {
    fn disassemble(&self) -> (&str, &str, &str, &str, &str) {
        let lib_cfg = LIB_CFG.read_recursive();

        let sort_tag_stem_copy_counter_ext = &self
            .file_name()
            .unwrap_or_default()
            .to_str()
            .unwrap_or_default();

        let sort_tag_stem_copy_counter = &self
            .file_stem()
            .unwrap_or_default()
            .to_str()
            .unwrap_or_default();

        let stem_copy_counter = sort_tag_stem_copy_counter.trim_start_matches(
            &lib_cfg
                .filename
                .sort_tag_chars
                .chars()
                .collect::<Vec<char>>()[..],
        );

        let sort_tag = &sort_tag_stem_copy_counter
            [0..sort_tag_stem_copy_counter.len() - stem_copy_counter.len()];

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

        let stem = Self::trim_copy_counter(stem_copy_counter);

        let copy_counter = &stem_copy_counter[stem.len()..];

        let ext = &self
            .extension()
            .unwrap_or_default()
            .to_str()
            .unwrap_or_default();
        (sort_tag, stem_copy_counter_ext, stem, copy_counter, ext)
    }

    /// Check if 2 filenames are equal. Compare all parts, except the copy counter.
    /// Consider 2 file identical even when they have a different copy counter.
    fn exclude_copy_counter_eq(&self, p2: &Path) -> bool {
        let (sort_tag1, _, stem1, _, ext1) = &self.disassemble();
        let (sort_tag2, _, stem2, _, ext2) = &p2.disassemble();
        sort_tag1 == sort_tag2 && stem1 == stem2 && ext1 == ext2
    }

    /// Check if a `path` points to a file with a
    /// "well formed" filename.
    /// We consider it well formed,
    /// * if `path` has no directory components, only
    ///   a filename, and
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

    /// Helper function that trims the copy counter at the end of string.
    /// If there is none, return the same.
    #[inline]
    fn trim_copy_counter(tag: &str) -> &str {
        let lib_cfg = LIB_CFG.read_recursive();
        // Strip closing brackets at the end.
        let tag1 =
            if let Some(t) = tag.strip_suffix(&lib_cfg.filename.copy_counter_closing_brackets) {
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
        let tag3 =
            if let Some(t) = tag2.strip_suffix(&lib_cfg.filename.copy_counter_opening_brackets) {
                t
            } else {
                return tag;
            };

        tag3
    }

    /// True if the filename extension is considered as a Tp-Note file.
    /// Checks if the filename extension is one of the following list
    /// taken from the configuration file:
    /// FILENAME_EXTENSIONS_MD, FILENAME_EXTENSIONS_RST, FILENAME_EXTENSIONS_HTML,
    /// FILENAME_EXTENSIONS_TXT, FILENAME_EXTENSIONS_NO_VIEWER
    fn has_tpnote_extension(&self) -> bool {
        self.has_wellformed_filename()
            && !matches!(MarkupLanguage::from(self), MarkupLanguage::None)
    }
}

#[cfg(test)]
mod tests {
    use super::NotePath;
    use super::NotePathBuf;
    use crate::config::LIB_CFG;
    use std::path::Path;
    use std::path::PathBuf;

    #[test]
    fn test_shorten_filename() {
        use std::ffi::OsString;
        use std::path::PathBuf;
        let lib_cfg = LIB_CFG.read_recursive();

        // Test concatenation of extra `-` if it ends with a copy counter pattern.
        let mut input = "fn".to_string();
        // This makes the filename problematic
        input.push_str(&lib_cfg.filename.copy_counter_opening_brackets);
        input.push('0');
        input.push_str(&lib_cfg.filename.copy_counter_closing_brackets);

        // We expect this to be corrected.
        let mut expected = input.clone();
        // Append '-'.
        expected.push_str(&lib_cfg.filename.copy_counter_extra_separator);

        input.push_str(".ext");
        expected.push_str(".ext");

        let mut input = PathBuf::from(input);
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
            "1_2_3-",
            "my_title--my_subtitle(1).md",
            "my_title--my_subtitle",
            "(1)",
            "md",
        );
        let p = Path::new("/my/dir/1_2_3-my_title--my_subtitle(1).md");
        let result = p.disassemble();
        assert_eq!(expected, result);

        let expected = (
            "2021.04.12-",
            "my_title--my_subtitle(1).md",
            "my_title--my_subtitle",
            "(1)",
            "md",
        );
        let p = Path::new("/my/dir/2021.04.12-my_title--my_subtitle(1).md");
        let result = p.disassemble();
        assert_eq!(expected, result);

        let expected = (
            "2021 04 12 ",
            "my_title--my_subtitle(1).md",
            "my_title--my_subtitle",
            "(1)",
            "md",
        );
        let p = Path::new("/my/dir/2021 04 12 my_title--my_subtitle(1).md");
        let result = p.disassemble();
        assert_eq!(expected, result);

        let expected = ("2021 04 12 ", "", "", "", "");
        let p = Path::new("/my/dir/2021 04 12 ");
        let result = p.disassemble();
        assert_eq!(expected, result);

        // This triggers the bug fixed with v1.14.3.
        let expected = ("2021 04 12 ", ".md", "", "", "md");
        let p = Path::new("/my/dir/2021 04 12 .md");
        let result = p.disassemble();
        assert_eq!(expected, result);

        let expected = ("2021 04 12 ", "(9).md", "", "(9)", "md");
        let p = Path::new("/my/dir/2021 04 12 (9).md");
        let result = p.disassemble();
        assert_eq!(expected, result);

        let expected = ("20221030-", "some.pdf--Note.md", "some.pdf--Note", "", "md");
        let p = Path::new("/my/dir/20221030-some.pdf--Note.md");
        let result = p.disassemble();
        assert_eq!(expected, result);
    }

    #[test]
    fn test_assemble_filename() {
        let expected = PathBuf::from("1_2_3-my_file-1-.md");
        let result = PathBuf::from_disassembled("1_2_3-", "my_file", "-1-", "md");
        assert_eq!(expected, result);
    }

    #[test]
    fn test_trim_copy_counter() {
        // Pattern found and removed.
        let expected = "my_stem";
        let result = Path::trim_copy_counter("my_stem(78)");
        assert_eq!(expected, result);

        // Pattern found and removed.
        let expected = "my_stem-";
        let result = Path::trim_copy_counter("my_stem-(78)");
        assert_eq!(expected, result);

        // Pattern found and removed.
        let expected = "my_stem_";
        let result = Path::trim_copy_counter("my_stem_(78)");
        assert_eq!(expected, result);

        // Pattern not found.
        assert_eq!(expected, result);
        let expected = "my_stem_(78))";
        let result = Path::trim_copy_counter("my_stem_(78))");
        assert_eq!(expected, result);

        // Pattern not found.
        let expected = "my_stem_)78)";
        let result = Path::trim_copy_counter("my_stem_)78)");
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
}
