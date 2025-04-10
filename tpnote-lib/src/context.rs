//! Extends the built-in Tera filters.
use tera::Value;

use crate::config::Assertion;
use crate::config::FILENAME_ROOT_PATH_MARKER;
use crate::config::LIB_CFG;
use crate::config::TMPL_VAR_CURRENT_SCHEME;
use crate::config::TMPL_VAR_DIR_PATH;
use crate::config::TMPL_VAR_EXTENSION_DEFAULT;
use crate::config::TMPL_VAR_FM_;
use crate::config::TMPL_VAR_FM_ALL;
use crate::config::TMPL_VAR_FM_SCHEME;
use crate::config::TMPL_VAR_LANG;
use crate::config::TMPL_VAR_PATH;
use crate::config::TMPL_VAR_ROOT_PATH;
use crate::config::TMPL_VAR_SCHEME_SYNC_DEFAULT;
use crate::config::TMPL_VAR_USERNAME;
use crate::content::Content;
use crate::error::LibCfgError;
use crate::error::NoteError;
use crate::filename::Extension;
use crate::filename::NotePath;
use crate::filename::NotePathStr;
use crate::filter::name;
use crate::front_matter::all_leaves;
use crate::front_matter::FrontMatter;
use crate::settings::SETTINGS;
use std::borrow::Cow;
use std::matches;
use std::ops::Deref;
use std::ops::DerefMut;
use std::path::Path;
use std::path::PathBuf;

/// Tiny wrapper around "Tera context" with some additional information.
#[derive(Clone, Debug, PartialEq)]
pub struct Context {
    /// Collection of substitution variables.
    ct: tera::Context,
    /// First positional command line argument.
    pub path: PathBuf,
    /// The directory (only) path corresponding to the first positional
    /// command line argument. The is our working directory and
    /// the directory where the note file is (will be) located.
    pub dir_path: PathBuf,
    /// `dir_path` is a subdirectory of `root_path`. `root_path` is the
    /// first directory, that upwards from `dir_path`, contains a file named
    /// `FILENAME_ROOT_PATH_MARKER` (or `/` if no marker file can be found).
    /// The root directory is interpreted by Tp-Note's viewer as its base
    /// directory: only files within this directory are served.
    pub root_path: PathBuf,
}

/// A thin wrapper around `tera::Context` storing some additional
/// information.
///
impl Context {
    /// Constructor: `path` is the first positional command line parameter
    /// `<path>` (see man page). `path` must point to a directory or
    /// a file.
    ///
    /// A copy of `path` is stored in `self.ct` as key `TMPL_VAR_PATH`. It
    /// directory path as key `TMPL_VAR_DIR_PATH`.
    ///
    /// ```rust
    /// use std::path::Path;
    /// use tpnote_lib::settings::set_test_default_settings;
    /// use tpnote_lib::config::TMPL_VAR_DIR_PATH;
    /// use tpnote_lib::config::TMPL_VAR_PATH;
    /// use tpnote_lib::context::Context;
    /// set_test_default_settings().unwrap();
    ///
    /// let mut context = Context::from(&Path::new("/path/to/mynote.md"));
    ///
    /// assert_eq!(context.path, Path::new("/path/to/mynote.md"));
    /// assert_eq!(context.dir_path, Path::new("/path/to/"));
    /// assert_eq!(&context.get(TMPL_VAR_PATH).unwrap().to_string(),
    ///             r#""/path/to/mynote.md""#);
    /// assert_eq!(&context.get(TMPL_VAR_DIR_PATH).unwrap().to_string(),
    ///             r#""/path/to""#);
    /// ```
    ///
    pub fn from(path: &Path) -> Self {
        let mut ct = tera::Context::new();
        let path = path.to_path_buf();

        // `dir_path` is a directory as fully qualified path, ending
        // by a separator.
        let dir_path = if path.is_dir() {
            path.clone()
        } else {
            path.parent()
                .unwrap_or_else(|| Path::new("./"))
                .to_path_buf()
        };

        // Get the root directory.
        let mut root_path = Path::new("");

        for anc in dir_path.ancestors() {
            root_path = anc;
            let mut p = anc.to_owned();
            p.push(Path::new(FILENAME_ROOT_PATH_MARKER));
            if p.is_file() {
                break;
            }
        }
        let root_path = root_path.to_owned();
        debug_assert!(dir_path.starts_with(&root_path));

        // Register the canonicalized fully qualified file name.
        ct.insert(TMPL_VAR_PATH, &path);
        ct.insert(TMPL_VAR_DIR_PATH, &dir_path);
        ct.insert(TMPL_VAR_ROOT_PATH, &root_path);

        // Insert environment.
        let mut context = Self {
            ct,
            path,
            dir_path,
            root_path,
        };
        context.insert_config_vars();
        context.insert_settings();
        context
    }

    /// Inserts the YAML front header variables in the context for later use
    /// with templates.
    ///
    pub(crate) fn insert_front_matter(&mut self, fm: &FrontMatter) {
        let mut fm_all_map = self
            .ct
            .remove(TMPL_VAR_FM_ALL)
            .and_then(|v| {
                if let tera::Value::Object(map) = v {
                    Some(map)
                } else {
                    None
                }
            })
            .unwrap_or_default();

        // Collect all localized scheme field names.
        // Example: `["scheme", "scheme", "Schema"]`
        let localized_scheme_names: Vec<String> = LIB_CFG
            .read_recursive()
            .scheme
            .iter()
            .map(|s| {
                s.tmpl
                    .fm_var
                    .localization
                    .iter()
                    .find_map(|(k, v)| (k == TMPL_VAR_FM_SCHEME).then_some(v.to_owned()))
            })
            .collect::<Option<Vec<String>>>()
            .unwrap_or_default();

        // Search for localized scheme names in front matter.
        // `(scheme_idx, field_value)`. Example: `(2, "Deutsch")`
        let localized_scheme: Option<(usize, &str)> = localized_scheme_names
            .iter()
            .enumerate()
            .find_map(|(i, k)| fm.0.get(k).and_then(|s| s.as_str()).map(|s| (i, s)));

        let scheme = if let Some((scheme, _)) = localized_scheme {
            {
                log::trace!(
                    "Using scheme field in front matter as current scheme: {:?}",
                    localized_scheme
                );
                scheme
            }
        } else {
            SETTINGS.read_recursive().current_scheme
        };
        let scheme = &LIB_CFG.read_recursive().scheme[scheme];

        let vars = &scheme.tmpl.fm_var.localization;
        for (key, value) in fm.iter() {
            // This delocalizes the variable name and prepends `fm_` to its name.
            // NB: We also insert `Value::Array` and `Value::Object`
            // variants, No flattening occurs here.
            let fm_key = vars.iter().find(|&l| &l.1 == key).map_or_else(
                || {
                    let mut s = TMPL_VAR_FM_.to_string();
                    s.push_str(key);
                    Cow::Owned(s)
                },
                |l| Cow::Borrowed(&l.0),
            );

            // Store a copy in `fm`.
            fm_all_map.insert(fm_key.to_string(), value.clone());
        }
        // Register the collection as `Object(Map<String, Value>)`.
        self.ct.insert(TMPL_VAR_FM_ALL, &fm_all_map);
    }

    /// Inserts clipboard or stdin data into the context. The data may
    /// contain some copied text with or without a YAML header. The latter
    /// usually carries front matter variable. These are added separately via
    /// `insert_front_matter()`. The `input` data below is registered with
    /// the key name given by `tmpl_var`. Typical names are `"clipboard"` or
    /// `"stdin"`. If the below `input` contains a valid YAML header, it will be
    /// registered in the context with the key name given by `tmpl_var_header`.
    /// This string is typically one of `clipboard_header` or `std_header`. The
    /// raw data that will be inserted into the context.
    ///
    /// ```rust
    /// use std::path::Path;
    /// use tpnote_lib::settings::set_test_default_settings;
    /// use tpnote_lib::context::Context;
    /// use tpnote_lib::content::Content;
    /// use tpnote_lib::content::ContentString;
    /// set_test_default_settings().unwrap();
    ///
    /// let mut context = Context::from(&Path::new("/path/to/mynote.md"));
    ///
    /// context.insert_content("clipboard", "clipboard_header",
    ///      &ContentString::from(String::from("Data from clipboard.")));
    /// assert_eq!(&context.get("clipboard").unwrap().to_string(),
    ///     "\"Data from clipboard.\"");
    ///
    /// context.insert_content("stdin", "stdin_header",
    ///      &ContentString::from("---\ntitle: \"My Stdin.\"\n---\nbody".to_string()));
    /// assert_eq!(&context.get("stdin").unwrap().to_string(),
    ///     r#""body""#);
    /// assert_eq!(&context.get("stdin_header").unwrap().to_string(),
    ///     r#""title: \"My Stdin.\"""#);
    /// // "fm_title" is dynamically generated from the header variable "title".
    /// assert_eq!(&context
    ///            .get("fm").unwrap()
    ///            .get("fm_title").unwrap().to_string(),
    ///     r#""My Stdin.""#);
    /// ```
    pub fn insert_content(
        &mut self,
        tmpl_var: &str,
        tmpl_var_header: &str,
        input: &impl Content,
    ) -> Result<(), NoteError> {
        // Register input.
        (*self).insert(tmpl_var_header, input.header());
        (*self).insert(tmpl_var, input.body());

        // Can we find a front matter in the input stream? If yes, the
        // unmodified input stream is our new note content.
        if !input.header().is_empty() {
            let input_fm = FrontMatter::try_from(input.header());
            match input_fm {
                Ok(ref fm) => {
                    log::trace!(
                        "Input stream from \"{}\" results in front matter:\n{:#?}",
                        tmpl_var,
                        &fm
                    )
                }
                Err(ref e) => {
                    if !input.header().is_empty() {
                        return Err(NoteError::InvalidInputYaml {
                            tmpl_var: tmpl_var.to_string(),
                            source_str: e.to_string(),
                        });
                    }
                }
            };

            // Register front matter.
            // The variables registered here can be overwrite the ones from the clipboard.
            if let Ok(fm) = input_fm {
                self.insert_front_matter(&fm);
            }
        }
        Ok(())
    }

    /// Captures Tp-Note's environment and stores it as variables in a
    /// `context` collection. The variables are needed later to populate
    /// a context template and a filename template.
    ///
    /// This function add the keys:
    ///
    /// * `TMPL_VAR_EXTENSION_DEFAULT`
    /// * `TMPL_VAR_USERNAME`
    /// * `TMPL_VAR_LANG`
    /// * `TMPL_VAR_CURRENT_SCHEME`
    ///
    /// ```
    /// use std::path::Path;
    /// use tpnote_lib::config::TMPL_VAR_EXTENSION_DEFAULT;
    /// use tpnote_lib::config::TMPL_VAR_CURRENT_SCHEME;
    /// use tpnote_lib::settings::set_test_default_settings;
    /// use tpnote_lib::context::Context;
    /// set_test_default_settings().unwrap();
    ///
    /// // The constructor calls `context.insert_settings()` before returning.
    /// let mut context = Context::from(&Path::new("/path/to/mynote.md"));
    ///
    /// // For most platforms `context.get("extension_default")` is `md`
    /// assert_eq!(&context.get(TMPL_VAR_EXTENSION_DEFAULT).unwrap().to_string(),
    ///     &format!("\"md\""));
    /// // `Settings.current_scheme` is by default the `default` scheme.
    /// assert_eq!(&context.get(TMPL_VAR_CURRENT_SCHEME).unwrap().to_string(),
    ///     &format!("\"default\""));
    /// ```
    fn insert_settings(&mut self) {
        let settings = SETTINGS.read_recursive();

        // Default extension for new notes as defined in the configuration file.
        (*self).insert(
            TMPL_VAR_EXTENSION_DEFAULT,
            settings.extension_default.as_str(),
        );

        {
            let lib_cfg = LIB_CFG.read_recursive();
            (*self).insert(
                TMPL_VAR_CURRENT_SCHEME,
                &lib_cfg.scheme[settings.current_scheme].name,
            );
        } // Release `lib_cfg` here.

        // Search for UNIX, Windows and MacOS user-names.
        (*self).insert(TMPL_VAR_USERNAME, &settings.author);

        // Get the user's language tag.
        (*self).insert(TMPL_VAR_LANG, &settings.lang);
    }

    /// Insert some configuration variables into the context so that they
    /// can be used in the templates.
    ///
    /// This function add the keys:
    /// TMPL_VAR_SCHEME_SYNC_DEFAULT.
    ///
    /// ```
    /// use std::path::Path;
    /// use tpnote_lib::config::TMPL_VAR_SCHEME_SYNC_DEFAULT;
    /// use tpnote_lib::settings::set_test_default_settings;
    /// use tpnote_lib::context::Context;
    /// set_test_default_settings().unwrap();
    ///
    /// // The constructor calls `context.insert_settings()` before returning.
    /// let mut context = Context::from(&Path::new("/path/to/mynote.md"));
    ///
    /// // When the note's YAML header does not contain a `scheme:` field,
    /// // the `default` scheme is used.
    /// assert_eq!(&context.get(TMPL_VAR_SCHEME_SYNC_DEFAULT).unwrap().to_string(),
    ///     &format!("\"default\""));
    /// ```
    fn insert_config_vars(&mut self) {
        let lib_cfg = LIB_CFG.read_recursive();

        // Default extension for new notes as defined in the configuration file.
        (*self).insert(
            TMPL_VAR_SCHEME_SYNC_DEFAULT,
            lib_cfg.scheme_sync_default.as_str(),
        );
    }

    /// Checks if the front matter variables satisfy preconditions.
    /// The path is the path to the current document.
    #[inline]
    pub fn assert_precoditions(&self) -> Result<(), NoteError> {
        let path = &self.path;
        let lib_cfg = &LIB_CFG.read_recursive();

        // Get front matter scheme if there is any.
        let fm_all = self.get(TMPL_VAR_FM_ALL);
        if fm_all.is_none() {
            return Ok(());
        }
        let fm_all = fm_all.unwrap();
        let fm_scheme = fm_all.get(TMPL_VAR_FM_SCHEME).and_then(|v| v.as_str());
        let scheme_idx = fm_scheme.and_then(|scheme_name| {
            lib_cfg
                .scheme
                .iter()
                .enumerate()
                .find_map(|(i, s)| (s.name == scheme_name).then_some(i))
        });
        // If not use `current_scheme` from `SETTINGS`
        let scheme_idx = scheme_idx.unwrap_or_else(|| SETTINGS.read_recursive().current_scheme);
        let scheme = &lib_cfg.scheme[scheme_idx];

        for (key, conditions) in scheme.tmpl.fm_var.assertions.iter() {
            if let Some(value) = fm_all.get(key) {
                for cond in conditions {
                    match cond {
                        Assertion::IsDefined => {}

                        Assertion::IsString => {
                            if !all_leaves(value, &|v| matches!(v, Value::String(..))) {
                                return Err(NoteError::FrontMatterFieldIsNotString {
                                    field_name: name(scheme, key).to_string(),
                                });
                            }
                        }

                        Assertion::IsNotEmptyString => {
                            if !all_leaves(value, &|v| {
                                matches!(v, Value::String(..)) && v.as_str() != Some("")
                            }) {
                                return Err(NoteError::FrontMatterFieldIsEmptyString {
                                    field_name: name(scheme, key).to_string(),
                                });
                            }
                        }

                        Assertion::IsNumber => {
                            if !all_leaves(value, &|v| matches!(v, Value::Number(..))) {
                                return Err(NoteError::FrontMatterFieldIsNotNumber {
                                    field_name: name(scheme, key).to_string(),
                                });
                            }
                        }

                        Assertion::IsBool => {
                            if !all_leaves(value, &|v| matches!(v, Value::Bool(..))) {
                                return Err(NoteError::FrontMatterFieldIsNotBool {
                                    field_name: name(scheme, key).to_string(),
                                });
                            }
                        }

                        Assertion::IsNotCompound => {
                            if matches!(value, Value::Array(..))
                                || matches!(value, Value::Object(..))
                            {
                                return Err(NoteError::FrontMatterFieldIsCompound {
                                    field_name: name(scheme, key).to_string(),
                                });
                            }
                        }

                        Assertion::IsValidSortTag => {
                            let fm_sort_tag = value.as_str().unwrap_or_default();
                            if !fm_sort_tag.is_empty() {
                                // Check for forbidden characters.
                                let (_, rest, is_sequential) = fm_sort_tag.split_sort_tag(true);
                                if !rest.is_empty() {
                                    return Err(NoteError::FrontMatterFieldIsInvalidSortTag {
                                        sort_tag: fm_sort_tag.to_owned(),
                                        sort_tag_extra_chars: scheme
                                            .filename
                                            .sort_tag
                                            .extra_chars
                                            .escape_default()
                                            .to_string(),
                                        filename_sort_tag_letters_in_succession_max: scheme
                                            .filename
                                            .sort_tag
                                            .letters_in_succession_max,
                                    });
                                }

                                // Check for duplicate sequential sort-tags.
                                if !is_sequential {
                                    // No further checks.
                                    return Ok(());
                                }
                                let docpath = path.to_str().unwrap_or_default();

                                let (dirpath, filename) =
                                    docpath.rsplit_once(['/', '\\']).unwrap_or(("", docpath));
                                let sort_tag = filename.split_sort_tag(false).0;
                                // No further check if filename(path) has no sort-tag
                                // or if sort-tags are identical.
                                if sort_tag.is_empty() || sort_tag == fm_sort_tag {
                                    return Ok(());
                                }
                                let dirpath = Path::new(dirpath);

                                if let Some(other_file) =
                                    dirpath.has_file_with_sort_tag(fm_sort_tag)
                                {
                                    return Err(NoteError::FrontMatterFieldIsDuplicateSortTag {
                                        sort_tag: fm_sort_tag.to_string(),
                                        existing_file: other_file,
                                    });
                                }
                            }
                        }

                        Assertion::IsTpnoteExtension => {
                            let file_ext = value.as_str().unwrap_or_default();

                            if !file_ext.is_empty() && !(*file_ext).is_tpnote_ext() {
                                return Err(NoteError::FrontMatterFieldIsNotTpnoteExtension {
                                    extension: file_ext.to_string(),
                                    extensions: {
                                        use std::fmt::Write;
                                        let mut errstr = scheme.filename.extensions.iter().fold(
                                            String::new(),
                                            |mut output, (k, _v1, _v2)| {
                                                let _ = write!(output, "{k}, ");
                                                output
                                            },
                                        );
                                        errstr.truncate(errstr.len().saturating_sub(2));
                                        errstr
                                    },
                                });
                            }
                        }

                        Assertion::IsConfiguredScheme => {
                            let fm_scheme = value.as_str().unwrap_or_default();
                            match lib_cfg.scheme_idx(fm_scheme) {
                                Ok(_) => {}
                                Err(LibCfgError::SchemeNotFound {
                                    scheme_name,
                                    schemes,
                                }) => {
                                    return Err(NoteError::SchemeNotFound {
                                        scheme_val: scheme_name,
                                        scheme_key: key.to_string(),
                                        schemes,
                                    })
                                }
                                Err(e) => return Err(e.into()),
                            };
                        }

                        Assertion::NoOperation => {}
                    } //
                }
                //
            } else if conditions.contains(&Assertion::IsDefined) {
                return Err(NoteError::FrontMatterFieldMissing {
                    field_name: name(scheme, key).to_string(),
                });
            }
        }
        Ok(())
    }
}

/// Auto dereferences for convenient access to `tera::Context`.
impl Deref for Context {
    type Target = tera::Context;

    fn deref(&self) -> &Self::Target {
        &self.ct
    }
}

/// Auto dereferences for convenient access to `tera::Context`.
impl DerefMut for Context {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.ct
    }
}

#[cfg(test)]
mod tests {

    use crate::{config::TMPL_VAR_FM_ALL, error::NoteError};
    use std::path::Path;

    #[test]
    fn test_insert_front_matter() {
        use crate::context::Context;
        use crate::front_matter::FrontMatter;
        use std::path::Path;
        let mut context = Context::from(Path::new("/path/to/mynote.md"));
        context
            .insert_front_matter(&FrontMatter::try_from("title: My Stdin.\nsome: text").unwrap());

        assert_eq!(
            &context
                .get(TMPL_VAR_FM_ALL)
                .unwrap()
                .get("fm_title")
                .unwrap()
                .to_string(),
            r#""My Stdin.""#
        );
        assert_eq!(
            &context
                .get(TMPL_VAR_FM_ALL)
                .unwrap()
                .get("fm_some")
                .unwrap()
                .to_string(),
            r#""text""#
        );
        assert_eq!(
            &context
                .get(TMPL_VAR_FM_ALL)
                .unwrap()
                .get("fm_title")
                .unwrap()
                .to_string(),
            r#""My Stdin.""#
        );
        assert_eq!(
            &context
                .get(TMPL_VAR_FM_ALL)
                .unwrap()
                .get("fm_some")
                .unwrap()
                .to_string(),
            r#""text""#
        );
    }

    #[test]
    fn test_insert_front_matter2() {
        use crate::context::Context;
        use crate::front_matter::FrontMatter;
        use std::path::Path;
        let mut context = Context::from(Path::new("/path/to/mynote.md"));
        context.insert_front_matter(&FrontMatter::try_from("title: My Stdin.").unwrap());

        context.insert_front_matter(&FrontMatter::try_from("some: text").unwrap());

        assert_eq!(
            &context
                .get(TMPL_VAR_FM_ALL)
                .unwrap()
                .get("fm_title")
                .unwrap()
                .to_string(),
            r#""My Stdin.""#
        );
        assert_eq!(
            &context
                .get(TMPL_VAR_FM_ALL)
                .unwrap()
                .get("fm_some")
                .unwrap()
                .to_string(),
            r#""text""#
        );
        assert_eq!(
            &context
                .get(TMPL_VAR_FM_ALL)
                .unwrap()
                .get("fm_title")
                .unwrap()
                .to_string(),
            r#""My Stdin.""#
        );
        assert_eq!(
            &context
                .get(TMPL_VAR_FM_ALL)
                .unwrap()
                .get("fm_some")
                .unwrap()
                .to_string(),
            r#""text""#
        );
    }

    #[test]
    fn test_assert_preconditions() {
        // Check `tmpl.filter.assert_preconditions` in
        // `tpnote_lib/src/config_default.toml` to understand these tests.
        use crate::context::Context;
        use crate::front_matter::FrontMatter;
        use serde_json::json;
        //
        // Is empty.
        let input = "";
        let fm = FrontMatter::try_from(input).unwrap();
        let mut cx = Context::from(Path::new("does not matter"));
        cx.insert_front_matter(&fm);

        assert!(matches!(
            cx.assert_precoditions().unwrap_err(),
            NoteError::FrontMatterFieldMissing { .. }
        ));

        //
        // Ok as long as no other file with that sort-tag exists.
        let input = "# document start
        title: The book
        sort_tag:    123b";
        let fm = FrontMatter::try_from(input).unwrap();
        let mut cx = Context::from(Path::new("./03b-test.md"));
        cx.insert_front_matter(&fm);

        assert!(matches!(cx.assert_precoditions(), Ok(())));

        //
        // Should not be a compound type.
        let input = "# document start
        title: The book
        sort_tag:
        -    1234
        -    456";
        let fm = FrontMatter::try_from(input).unwrap();
        let mut cx = Context::from(Path::new("does not matter"));
        cx.insert_front_matter(&fm);
        assert!(matches!(
            cx.assert_precoditions().unwrap_err(),
            NoteError::FrontMatterFieldIsCompound { .. }
        ));

        //
        // Should not be a compound type.
        let input = "# document start
        title: The book
        sort_tag:
          first:  1234
          second: 456";
        let fm = FrontMatter::try_from(input).unwrap();
        let mut cx = Context::from(Path::new("does not matter"));
        cx.insert_front_matter(&fm);
        assert!(matches!(
            cx.assert_precoditions().unwrap_err(),
            NoteError::FrontMatterFieldIsCompound { .. }
        ));

        //
        // Not registered file extension.
        let input = "# document start
        title: The book
        file_ext:    xyz";
        let fm = FrontMatter::try_from(input).unwrap();
        let mut cx = Context::from(Path::new("does not matter"));
        cx.insert_front_matter(&fm);
        assert!(matches!(
            cx.assert_precoditions().unwrap_err(),
            NoteError::FrontMatterFieldIsNotTpnoteExtension { .. }
        ));

        //
        // Check `bool`
        let input = "# document start
        title: The book
        filename_sync: error, here should be a bool";
        let fm = FrontMatter::try_from(input).unwrap();
        let mut cx = Context::from(Path::new("does not matter"));
        cx.insert_front_matter(&fm);
        assert!(matches!(
            cx.assert_precoditions().unwrap_err(),
            NoteError::FrontMatterFieldIsNotBool { .. }
        ));

        //
        let input = "# document start
        title: my title
        subtitle: my subtitle
        ";
        let expected = json!({"fm_title": "my title", "fm_subtitle": "my subtitle"});

        let fm = FrontMatter::try_from(input).unwrap();
        let mut cx = Context::from(Path::new("does not matter"));
        cx.insert_front_matter(&fm);
        assert_eq!(cx.get(TMPL_VAR_FM_ALL).unwrap(), &expected);

        //
        let input = "# document start
        title: my title
        file_ext: ''
        ";
        let expected = json!({"fm_title": "my title", "fm_file_ext": ""});

        let fm = FrontMatter::try_from(input).unwrap();
        let mut cx = Context::from(Path::new("does not matter"));
        cx.insert_front_matter(&fm);
        assert_eq!(cx.get(TMPL_VAR_FM_ALL).unwrap(), &expected);

        //
        let input = "# document start
        title: ''
        subtitle: my subtitle
        ";
        let fm = FrontMatter::try_from(input).unwrap();
        let mut cx = Context::from(Path::new("does not matter"));
        cx.insert_front_matter(&fm);
        assert!(matches!(
            cx.assert_precoditions().unwrap_err(),
            NoteError::FrontMatterFieldIsEmptyString { .. }
        ));

        //
        let input = "# document start
        title: My doc
        author: 
        - First author
        - Second author
        ";
        let fm = FrontMatter::try_from(input).unwrap();
        let mut cx = Context::from(Path::new("does not matter"));
        cx.insert_front_matter(&fm);
        assert!(cx.assert_precoditions().is_ok());

        //
        let input = "# document start
        title: My doc
        subtitle: my subtitle
        author:
        - First title
        - 1234
        ";
        let fm = FrontMatter::try_from(input).unwrap();
        let mut cx = Context::from(Path::new("does not matter"));
        cx.insert_front_matter(&fm);
        assert!(matches!(
            cx.assert_precoditions().unwrap_err(),
            NoteError::FrontMatterFieldIsNotString { .. }
        ));
    }
}
