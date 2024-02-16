//! Helper functions dealing with filenames.
use crate::config::FILENAME_COPY_COUNTER_MAX;
use crate::config::FILENAME_DOTFILE_MARKER;
use crate::config::FILENAME_EXTENSION_SEPARATOR_DOT;
use crate::config::FILENAME_LEN_MAX;
use crate::config::LIB_CFG;
use crate::error::FileError;
use crate::markup_language::MarkupLanguage;
use crate::settings::SETTINGS;
use std::mem::swap;
use std::path::Path;
use std::path::PathBuf;
use std::time::SystemTime;

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
        let scheme = &LIB_CFG.read_recursive().scheme[SETTINGS.read_recursive().current_scheme];

        if !sort_tag.is_empty() {
            filename.push_str(sort_tag);
            filename.push_str(&scheme.filename.sort_tag.separator);
        }
        // Does the beginning of `stem` look like a sort-tag?
        // Make sure, that the path can not be misinterpreted, even if a
        // `sort_tag.separator` would follow.
        let mut test_path = String::from(stem);
        test_path.push_str(&scheme.filename.sort_tag.separator);
        // Do we need an `extra_separator`?
        if stem.is_empty() || !&test_path.split_sort_tag(false).0.is_empty() {
            filename.push(scheme.filename.sort_tag.extra_separator);
        }

        filename.push_str(stem);

        if let Some(cc) = copy_counter {
            // Is `copy_counter.extra_separator` necessary?
            // Does this stem ending look similar to a copy counter?
            if stem.split_copy_counter().1.is_some() {
                // Add an additional separator.
                filename.push_str(&scheme.filename.copy_counter.extra_separator);
            };

            filename.push_str(&scheme.filename.copy_counter.opening_brackets);
            filename.push_str(&cc.to_string());
            filename.push_str(&scheme.filename.copy_counter.closing_brackets);
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
        if stem_short.split_copy_counter().1.is_some() {
            let scheme = &LIB_CFG.read_recursive().scheme[SETTINGS.read_recursive().current_scheme];

            stem_short.push_str(&scheme.filename.copy_counter.extra_separator);
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

/// Extents `Path` with methods dealing with paths to Tp-Note files.
pub trait NotePath {
    /// Helper function that decomposes a fully qualified path name
    /// into (`sort_tag`, `stem_copy_counter_ext`, `stem`, `copy_counter`, `ext`).
    /// All sort-tag separators and copy-counter separators/brackets are removed.
    fn disassemble(&self) -> (&str, &str, &str, Option<usize>, &str);

    /// Compares with another `Path` to a Tp-Note file. They are considered equal
    /// even when the copy counter is different.
    fn exclude_copy_counter_eq(&self, p2: &Path) -> bool;

    /// Compare to all file extensions Tp-Note can open.
    fn has_tpnote_ext(&self) -> bool;

    /// Check if a `Path` points to a file with a "well-formed" filename.
    fn has_wellformed_filename(&self) -> bool;

    /// Get the filename of the last created Tp-Note file in the directory
    /// `self`. If more files have the same creation date, choose the
    /// lexicographical last sort-tag in the current directory. Files without
    /// sort tag are ignored.
    /// <https://doc.rust-lang.org/std/cmp/trait.Ord.html#lexicographical-comparison>
    fn find_last_created_file(&self) -> Option<String>;

    /// Checks if the directory in `self` has a Tp-Note file starting with the
    /// `sort_tag`. If found, return the filename, otherwise `None`
    fn has_file_with_sort_tag(&self, sort_tag: &str) -> Option<String>;

    /// A method that searches the directory in `self` for a Tp-Note
    /// file with the sort-tag `sort_tag`. It returns the filename.
    fn find_file_with_sort_tag(&self, sort_tag: &str) -> Option<PathBuf>;
}

impl NotePath for Path {
    fn disassemble(&self) -> (&str, &str, &str, Option<usize>, &str) {
        let sort_tag_stem_copy_counter_ext = self
            .file_name()
            .unwrap_or_default()
            .to_str()
            .unwrap_or_default();

        let (sort_tag, stem_copy_counter_ext, _) =
            sort_tag_stem_copy_counter_ext.split_sort_tag(false);

        let ext = Path::new(stem_copy_counter_ext)
            .extension()
            .unwrap_or_default()
            .to_str()
            .unwrap_or_default(); // Trim `sort_tag`.

        let (stem_copy_counter, ext) = if !ext.is_empty()
            && ext.chars().all(|c| c.is_ascii_alphanumeric())
        {
            (
                // This is a little faster than `stem_copy_counter_ext.file_stem()`.
                &stem_copy_counter_ext[..stem_copy_counter_ext.len().saturating_sub(ext.len() + 1)],
                // `ext` is Ok, we keep it.
                ext,
            )
        } else {
            (stem_copy_counter_ext, "")
        };

        let (stem, copy_counter) = stem_copy_counter.split_copy_counter();

        (sort_tag, stem_copy_counter_ext, stem, copy_counter, ext)
    }

    /// Check if 2 filenames are equal. Compare all parts, except the copy counter.
    /// Consider 2 file identical even when they have a different copy counter.
    fn exclude_copy_counter_eq(&self, p2: &Path) -> bool {
        let (sort_tag1, _, stem1, _, ext1) = self.disassemble();
        let (sort_tag2, _, stem2, _, ext2) = p2.disassemble();
        sort_tag1 == sort_tag2 && stem1 == stem2 && ext1 == ext2
    }

    /// Returns `True` if the path in `self` ends with an extension, that Tp-
    /// Note considers as it's own file. To do so, the extension is compared
    /// to all items in the registered `filename.extensions` table in the
    /// configuration file.
    fn has_tpnote_ext(&self) -> bool {
        MarkupLanguage::from(self).is_some()
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
            // Only accept extensions with alphanumeric characters.
            && ext.chars().all(|c| c.is_ascii_alphanumeric());

        is_filename && (is_dot_file || has_extension)
    }

    fn find_last_created_file(&self) -> Option<String> {
        if let Ok(files) = self.read_dir() {
            // If more than one file starts with `sort_tag`, retain the
            // alphabetic first.
            let mut filename_max = String::new();
            let mut ctime_max = SystemTime::UNIX_EPOCH;
            for file in files.flatten() {
                match file.file_type() {
                    Ok(ft) if ft.is_file() => {}
                    _ => continue,
                }
                let ctime = file
                    .metadata()
                    .ok()
                    .and_then(|md| md.created().ok())
                    .unwrap_or(SystemTime::UNIX_EPOCH);
                let filename = file.file_name();
                let filename = filename.to_str().unwrap();
                if filename.is_empty() || !filename.has_tpnote_ext() {
                    continue;
                }

                if ctime > ctime_max
                    || (ctime == ctime_max
                        && filename.split_sort_tag(false).0 > filename_max.split_sort_tag(false).0)
                {
                    filename_max = filename.to_string();
                    ctime_max = ctime;
                }
            } // End of loop.
              // Found, return result
            if !filename_max.is_empty() {
                Some(filename_max.to_string())
            } else {
                None
            }
        } else {
            None
        }
    }

    fn has_file_with_sort_tag(&self, sort_tag: &str) -> Option<String> {
        if let Ok(files) = self.read_dir() {
            for file in files.flatten() {
                match file.file_type() {
                    Ok(ft) if ft.is_file() => {}
                    _ => continue,
                }
                let filename = file.file_name();
                let filename = filename.to_str().unwrap();

                // Tests in the order of the cost.
                if filename.starts_with(sort_tag)
                    && filename.has_tpnote_ext()
                    && filename.split_sort_tag(false).0 == sort_tag
                {
                    let filename = filename.to_string();
                    return Some(filename);
                }
            }
        }
        None
    }

    fn find_file_with_sort_tag(&self, sort_tag: &str) -> Option<PathBuf> {
        let mut found = None;

        if let Ok(files) = self.read_dir() {
            // If more than one file starts with `sort_tag`, retain the
            // alphabetic first.
            let mut minimum = PathBuf::new();
            'file_loop: for file in files.flatten() {
                match file.file_type() {
                    Ok(ft) if ft.is_file() => {}
                    _ => continue,
                }
                let file = file.path();
                if !(*file).has_tpnote_ext() {
                    continue 'file_loop;
                }
                // Does this sort-tag short link correspond to
                // any sort-tag of a file in the same directory?
                if file.disassemble().0 == sort_tag {
                    // Before the first assignment `minimum` is empty.
                    // Finds the minimum.
                    if minimum == Path::new("") || minimum > file {
                        minimum = file;
                    }
                }
            } // End of loop.
            if minimum != Path::new("") {
                log::debug!(
                    "File `{}` referenced by sort-tag match `{}`.",
                    minimum.to_str().unwrap_or_default(),
                    sort_tag,
                );
                // Found, return result
                found = Some(minimum)
            }
        }
        found
    }
}

/// Some private helper functions related to note filenames.
pub(crate) trait NotePathStr {
    /// Returns `True` is the path in `self` ends with an extension, that
    /// registered as Tp-Note extension in `filename.extensions`.
    /// The input may contain a path as long as it ends with a filename.
    fn has_tpnote_ext(&self) -> bool;

    /// Helper function that expects a filename in `self` und
    /// matches the copy counter at the end of string,
    /// returns the result and the copy counter.
    /// This function removes all brackets and a potential extra separator.
    /// The input must not contain a path, only a filename is allowed here.
    fn split_copy_counter(&self) -> (&str, Option<usize>);

    /// Helper function that expects a filename in `self`:
    /// Greedily match sort tag chars and return it as a subslice as first tuple
    /// and the rest as second tuple: `(sort-tag, rest, is_sequential)`.
    /// The input must not contain a path, only a filename is allowed here.
    /// If `filename.sort_tag.separator` is defined, it must appear after the
    /// sort-tag (without being part of it). Otherwise the sort-tag is discarded.
    /// A sort-tag can not contain more than
    /// `FILENAME_SORT_TAG_LETTERS_IN_SUCCESSION_MAX` lowercase letters in a row.
    /// If `ignore_sort_tag_separator=true` this split runs with the setting
    /// `filename_sort_tag_separator=""`.
    /// If the boolean return value is true, the sort-tag satisfies the
    /// criteria for a sequential sort-tag.
    fn split_sort_tag(&self, ignore_sort_tag_separator: bool) -> (&str, &str, bool);

    /// Check and return the filename in `self`, if it contains only
    /// `lib_cfg.filename.sort_tag.extra_chars` (no sort-tag separator, no file
    /// stem, no extension). The number of lowercase letters in a row must not
    /// exceed `filename.sort_tag.letters_in_succession_max`.
    /// The input may contain a path as long as it ends with `/`, `\\` or a
    /// filename. The path, if present, it is ignored.
    fn is_valid_sort_tag(&self) -> Option<&str>;
}

impl NotePathStr for str {
    fn has_tpnote_ext(&self) -> bool {
        MarkupLanguage::from(Path::new(self)).is_some()
    }

    #[inline]
    fn split_copy_counter(&self) -> (&str, Option<usize>) {
        let scheme = &LIB_CFG.read_recursive().scheme[SETTINGS.read_recursive().current_scheme];
        // Strip closing brackets at the end.
        let tag1 =
            if let Some(t) = self.strip_suffix(&scheme.filename.copy_counter.closing_brackets) {
                t
            } else {
                return (self, None);
            };
        // Now strip numbers.
        let tag2 = tag1.trim_end_matches(|c: char| c.is_numeric());
        let copy_counter: Option<usize> = if tag2.len() < tag1.len() {
            tag1[tag2.len()..].parse().ok()
        } else {
            return (self, None);
        };
        // And finally strip starting bracket.
        let tag3 =
            if let Some(t) = tag2.strip_suffix(&scheme.filename.copy_counter.opening_brackets) {
                t
            } else {
                return (self, None);
            };
        // This is optional
        if let Some(t) = tag3.strip_suffix(&scheme.filename.copy_counter.extra_separator) {
            (t, copy_counter)
        } else {
            (tag3, copy_counter)
        }
    }

    fn split_sort_tag(&self, ignore_sort_tag_separator: bool) -> (&str, &str, bool) {
        let scheme = &LIB_CFG.read_recursive().scheme[SETTINGS.read_recursive().current_scheme];

        let mut is_sequential_sort_tag = true;

        let mut digits: u8 = 0;
        let mut letters: u8 = 0;
        let mut sort_tag = &self[..self
            .chars()
            .take_while(|&c| {
                if c.is_ascii_digit() {
                    digits += 1;
                    if digits > scheme.filename.sort_tag.sequential.digits_in_succession_max {
                        is_sequential_sort_tag = false;
                    }
                } else {
                    digits = 0;
                }

                if c.is_ascii_lowercase() {
                    letters += 1;
                } else {
                    letters = 0;
                }

                letters <= scheme.filename.sort_tag.letters_in_succession_max
                    && (c.is_ascii_digit()
                        || c.is_ascii_lowercase()
                        || scheme.filename.sort_tag.extra_chars.contains([c]))
            })
            .count()];

        let mut stem_copy_counter_ext;
        if scheme.filename.sort_tag.separator.is_empty() || ignore_sort_tag_separator {
            // `sort_tag` is correct.
            stem_copy_counter_ext = &self[sort_tag.len()..];
        } else {
            // Take `sort_tag.separator` into account.
            if let Some(i) = sort_tag.rfind(&scheme.filename.sort_tag.separator) {
                sort_tag = &sort_tag[..i];
                stem_copy_counter_ext = &self[i + scheme.filename.sort_tag.separator.len()..];
            } else {
                sort_tag = "";
                stem_copy_counter_ext = self;
            }
        }

        // Remove `sort_tag.extra_separator` if it is at the first position
        // followed by a `sort_tag_char` at the second position.
        let mut chars = stem_copy_counter_ext.chars();
        if chars
            .next()
            .is_some_and(|c| c == scheme.filename.sort_tag.extra_separator)
            && chars.next().is_some_and(|c| {
                c.is_ascii_digit()
                    || c.is_ascii_lowercase()
                    || scheme.filename.sort_tag.extra_chars.contains(c)
            })
        {
            stem_copy_counter_ext = stem_copy_counter_ext
                .strip_prefix(scheme.filename.sort_tag.extra_separator)
                .unwrap();
        }

        (sort_tag, stem_copy_counter_ext, is_sequential_sort_tag)
    }

    fn is_valid_sort_tag(&self) -> Option<&str> {
        let filename = if let Some((_, filename)) = self.rsplit_once(['\\', '/']) {
            filename
        } else {
            self
        };
        if filename.is_empty() {
            return None;
        }

        // If the rest is empty, all characters are in `sort_tag`.
        if filename.split_sort_tag(true).1.is_empty() {
            Some(filename)
        } else {
            None
        }
    }
}

/// A trait that interprets the implementing type as filename extension.
pub(crate) trait Extension {
    /// Returns `True` if `self` is equal to one of the Tp-Note extensions
    /// registered in the configuration file `filename.extensions` table.
    fn is_tpnote_ext(&self) -> bool;
}

impl Extension for str {
    fn is_tpnote_ext(&self) -> bool {
        MarkupLanguage::from(self).is_some()
    }
}

#[cfg(test)]
mod tests {
    use std::ffi::OsString;
    use std::path::Path;
    use std::path::PathBuf;

    #[test]
    fn test_from_disassembled() {
        use crate::filename::NotePathBuf;

        let expected = PathBuf::from("My_file.md");
        let result = PathBuf::from_disassembled("", "My_file", None, "md");
        assert_eq!(expected, result);

        let expected = PathBuf::from("1_2_3-My_file(1).md");
        let result = PathBuf::from_disassembled("1_2_3", "My_file", Some(1), "md");
        assert_eq!(expected, result);

        let expected = PathBuf::from("1_2_3-123 my_file(1).md");
        let result = PathBuf::from_disassembled("1_2_3", "123 my_file", Some(1), "md");
        assert_eq!(expected, result);

        let expected = PathBuf::from("1_2_3-'123-My_file(1).md");
        let result = PathBuf::from_disassembled("1_2_3", "123-My_file", Some(1), "md");
        assert_eq!(expected, result);

        let expected = PathBuf::from("'123-My_file(1).md");
        let result = PathBuf::from_disassembled("", "123-My_file", Some(1), "md");
        assert_eq!(expected, result);

        let res = PathBuf::from_disassembled("1234", "title--subtitle", Some(9), "md");
        assert_eq!(res, Path::new("1234-title--subtitle(9).md"));

        let res = PathBuf::from_disassembled("1234ab", "title--subtitle", Some(9), "md");
        assert_eq!(res, Path::new("1234ab-title--subtitle(9).md"));

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
    fn test_set_next_unused() {
        use crate::filename::NotePathBuf;

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
    fn test_shorten_filename() {
        use crate::config::FILENAME_LEN_MAX;
        use crate::filename::NotePathBuf;

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
        let mut input = "X".repeat(FILENAME_LEN_MAX + 10);
        input.push_str(".ext");

        let mut expected = "X".repeat(FILENAME_LEN_MAX - ".ext".len() - 1);
        expected.push_str(".ext");

        let mut input = PathBuf::from(input);
        input.shorten_filename();
        let output = input;
        assert_eq!(OsString::from(expected), output);
    }

    #[test]
    fn test_disassemble_filename() {
        use crate::filename::NotePath;

        let expected = (
            "1_2_3",
            "my_title--my_subtitle(1).md",
            "my_title--my_subtitle",
            Some(1),
            "md",
        );
        let p = Path::new("/my/dir/1_2_3-my_title--my_subtitle(1).md");
        let result = p.disassemble();
        assert_eq!(result, expected);

        let expected = (
            "1_2_3",
            "my_title--my_subtitle(1)-(9).md",
            "my_title--my_subtitle(1)",
            Some(9),
            "md",
        );
        let p = Path::new("/my/dir/1_2_3-my_title--my_subtitle(1)-(9).md");
        let result = p.disassemble();
        assert_eq!(result, expected);

        let expected = (
            "2021.04.12",
            "my_title--my_subtitle(1).md",
            "my_title--my_subtitle",
            Some(1),
            "md",
        );
        let p = Path::new("/my/dir/2021.04.12-my_title--my_subtitle(1).md");
        let result = p.disassemble();
        assert_eq!(result, expected);

        let expected = (
            "",
            "2021 04 12 my_title--my_subtitle(1).md",
            "2021 04 12 my_title--my_subtitle",
            Some(1),
            "md",
        );
        let p = Path::new("/my/dir/2021 04 12 my_title--my_subtitle(1).md");
        let result = p.disassemble();
        assert_eq!(result, expected);

        let expected = ("2021-04-12", "", "", None, "");
        let p = Path::new("/my/dir/2021-04-12-");
        let result = p.disassemble();
        assert_eq!(result, expected);

        // This triggers the bug fixed with v1.14.3.
        let expected = ("2021-04-12", ".dotfile", ".dotfile", None, "");
        let p = Path::new("/my/dir/2021-04-12-'.dotfile");
        let result = p.disassemble();
        assert_eq!(result, expected);

        let expected = ("2021-04-12", "(9).md", "", Some(9), "md");
        let p = Path::new("/my/dir/2021-04-12-(9).md");
        let result = p.disassemble();
        assert_eq!(result, expected);

        let expected = (
            "20221030",
            "Some.pdf--Note.md",
            "Some.pdf--Note",
            None,
            "md",
        );
        let p = Path::new("/my/dir/20221030-Some.pdf--Note.md");
        let result = p.disassemble();
        assert_eq!(result, expected);

        let expected = (
            "1_2_3",
            "my_title--my_subtitle(1).md",
            "my_title--my_subtitle",
            Some(1),
            "md",
        );
        let p = Path::new("/my/dir/1_2_3-my_title--my_subtitle(1).md");
        let result = p.disassemble();
        assert_eq!(result, expected);

        let expected = (
            "1_2_3",
            "123 my_title--my_subtitle(1).md",
            "123 my_title--my_subtitle",
            Some(1),
            "md",
        );
        let p = Path::new("/my/dir/1_2_3-123 my_title--my_subtitle(1).md");
        let result = p.disassemble();
        assert_eq!(result, expected);

        let expected = (
            "1_2_3-123",
            "My_title--my_subtitle(1).md",
            "My_title--my_subtitle",
            Some(1),
            "md",
        );
        let p = Path::new("/my/dir/1_2_3-123-My_title--my_subtitle(1).md");
        let result = p.disassemble();
        assert_eq!(result, expected);

        let expected = (
            "1_2_3",
            "123-my_title--my_subtitle(1).md",
            "123-my_title--my_subtitle",
            Some(1),
            "md",
        );
        let p = Path::new("/my/dir/1_2_3-'123-my_title--my_subtitle(1).md");
        let result = p.disassemble();
        assert_eq!(result, expected);

        let expected = (
            "1_2_3",
            "123 my_title--my_subtitle(1).md",
            "123 my_title--my_subtitle",
            Some(1),
            "md",
        );
        let p = Path::new("/my/dir/1_2_3-123 my_title--my_subtitle(1).md");
        let result = p.disassemble();
        assert_eq!(result, expected);

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
            "1a2b3ab",
            "my_title--my_subtitle(1).md",
            "my_title--my_subtitle",
            Some(1),
            "md",
        );
        let p = Path::new("/my/dir/1a2b3ab-my_title--my_subtitle(1).md");
        let result = p.disassemble();
        assert_eq!(expected, result);

        let expected = (
            "",
            "1a2b3abc-my_title--my_subtitle(1).md",
            "1a2b3abc-my_title--my_subtitle",
            Some(1),
            "md",
        );
        let p = Path::new("/my/dir/1a2b3abc-my_title--my_subtitle(1).md");
        let result = p.disassemble();
        assert_eq!(result, expected);

        let expected = (
            "1_2_3",
            "my_title--my_subtitle(1).m d",
            "my_title--my_subtitle(1).m d",
            None,
            "",
        );
        let p = Path::new("/my/dir/1_2_3-my_title--my_subtitle(1).m d");
        let result = p.disassemble();
        assert_eq!(result, expected);

        let expected = (
            "1_2_3",
            "my_title--my_subtitle(1)",
            "my_title--my_subtitle",
            Some(1),
            "",
        );
        let p = Path::new("/my/dir/1_2_3-my_title--my_subtitle(1)");
        let result = p.disassemble();
        assert_eq!(result, expected);
    }

    #[test]
    fn test_exclude_copy_counter_eq() {
        use crate::filename::NotePath;

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
    fn test_note_path_has_tpnote_ext() {
        use crate::filename::NotePath;

        //
        let path = Path::new("/dir/file.md");
        assert!(path.has_tpnote_ext());

        //
        let path = Path::new("/dir/file.abc");
        assert!(!path.has_tpnote_ext());

        // This goes wrong because a file path or at least a filename is
        // expected here.
        let path = Path::new("md");
        assert!(!path.has_tpnote_ext());
    }

    #[test]
    fn test_has_wellformed() {
        use crate::filename::NotePath;
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
    fn test_trim_copy_counter() {
        use crate::filename::NotePathStr;

        // Pattern found and removed.
        let expected = ("my_stem", Some(78));
        let result = "my_stem(78)".split_copy_counter();
        assert_eq!(expected, result);

        // Pattern found and removed.
        let expected = ("my_stem", Some(78));
        let result = "my_stem-(78)".split_copy_counter();
        assert_eq!(expected, result);

        // Pattern found and removed.
        let expected = ("my_stem_", Some(78));
        let result = "my_stem_(78)".split_copy_counter();
        assert_eq!(expected, result);

        // Pattern not found.
        assert_eq!(expected, result);
        let expected = ("my_stem_(78))", None);
        let result = "my_stem_(78))".split_copy_counter();
        assert_eq!(expected, result);

        // Pattern not found.
        let expected = ("my_stem_)78)", None);
        let result = "my_stem_)78)".split_copy_counter();
        assert_eq!(expected, result);
    }

    #[test]
    fn test_split_sort_tag() {
        use crate::filename::NotePathStr;

        let expected = ("123", "", true);
        let result = "123".split_sort_tag(true);
        assert_eq!(expected, result);

        let expected = ("123", "Rest", true);
        let result = "123-Rest".split_sort_tag(false);
        assert_eq!(expected, result);

        let expected = ("2023-10-30", "Rest", false);
        let result = "2023-10-30-Rest".split_sort_tag(false);
        assert_eq!(expected, result);
    }

    #[test]
    fn test_note_path_str_has_tpnote() {
        use crate::filename::NotePathStr;

        //
        let path_str = "/dir/file.md";
        assert!(path_str.has_tpnote_ext());

        //
        let path_str = "/dir/file.abc";
        assert!(!path_str.has_tpnote_ext());
    }

    #[test]
    fn test_is_tpnote_ext() {
        use crate::filename::Extension;
        //
        let ext = "md";
        assert!(ext.is_tpnote_ext());

        // This goes wrong because only `md` is expected here.
        let ext = "/dir/file.md";
        assert!(!ext.is_tpnote_ext());
    }

    #[test]
    fn test_filename_is_valid_sort_tag() {
        use super::NotePathStr;
        let f = "20230821";
        assert_eq!(f.is_valid_sort_tag(), Some("20230821"));

        let f = "dir/20230821";
        assert_eq!(f.is_valid_sort_tag(), Some("20230821"));

        let f = "dir\\20230821";
        assert_eq!(f.is_valid_sort_tag(), Some("20230821"));

        let f = "1_3_2";
        assert_eq!(f.is_valid_sort_tag(), Some("1_3_2"));

        let f = "1c2";
        assert_eq!(f.is_valid_sort_tag(), Some("1c2"));

        let f = "2023ab";
        assert_eq!(f.is_valid_sort_tag(), Some("2023ab"));

        let f = "2023abc";
        assert_eq!(f.is_valid_sort_tag(), None);

        let f = "dir/2023abc";
        assert_eq!(f.is_valid_sort_tag(), None);

        let f = "2023A";
        assert_eq!(f.is_valid_sort_tag(), None);

        let f = "20230821";
        assert_eq!(f.is_valid_sort_tag(), Some("20230821"));

        let f = "2023-08-21";
        assert_eq!(f.is_valid_sort_tag(), Some("2023-08-21"));

        let f = "20-08-21";
        assert_eq!(f.is_valid_sort_tag(), Some("20-08-21"));

        let f = "2023ab";
        assert_eq!(f.is_valid_sort_tag(), Some("2023ab"));

        let f = "202ab";
        assert_eq!(f.is_valid_sort_tag(), Some("202ab"));
    }
}
