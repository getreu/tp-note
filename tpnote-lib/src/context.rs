//! Extends the built-in Tera filters.
use crate::config::FILENAME_ROOT_PATH_MARKER;
use crate::config::LIB_CFG;
use crate::config::SETTINGS;
use crate::config::TMPL_VAR_DIR_PATH;
use crate::config::TMPL_VAR_EXTENSION_DEFAULT;
use crate::config::TMPL_VAR_FM_;
use crate::config::TMPL_VAR_FM_ALL;
use crate::config::TMPL_VAR_LANG;
use crate::config::TMPL_VAR_PATH;
use crate::config::TMPL_VAR_ROOT_PATH;
use crate::config::TMPL_VAR_USERNAME;
use crate::content::Content;
use crate::error::NoteError;
use crate::front_matter::FrontMatter;
use std::ops::Deref;
use std::ops::DerefMut;
use std::path::Path;
use std::path::PathBuf;

#[cfg(target_family = "windows")]
use windows_sys::Win32::Globalization::GetUserDefaultLocaleName;
#[cfg(target_family = "windows")]
use windows_sys::Win32::System::SystemServices::LOCALE_NAME_MAX_LENGTH;

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
    /// `FILENAME_ROOT_PATH_MARKER`. The root directory is interpreted by Tp-
    /// Note's viewer as its base directory: only files within this directory
    /// are served.
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
    /// use tpnote_lib::config::TMPL_VAR_DIR_PATH;
    /// use tpnote_lib::config::TMPL_VAR_PATH;
    /// use tpnote_lib::context::Context;
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
    /// Furthermore, the constructor captures _Tp-Note_'s environment
    /// and stores it as variables in a
    /// `context` collection. The variables are needed later to populate
    /// a context template and a filename template.
    ///
    /// This function add the keys:
    /// TMPL_VAR_EXTENSION_DEFAULT, TMPL_VAR_USERNAME and TMPL_VAR_LANG.
    ///
    /// ```
    /// use std::path::Path;
    /// use tpnote_lib::context::Context;
    /// use tpnote_lib::config::TMPL_VAR_EXTENSION_DEFAULT; // `extension_default`
    /// use tpnote_lib::config::FILENAME_EXTENSION_DEFAULT; // usually `md`
    /// let mut context = Context::from(&Path::new("/path/to/mynote.md"));
    ///
    /// // For most platforms `context.get("extension_default")` is `md`
    /// assert_eq!(&context.get(TMPL_VAR_EXTENSION_DEFAULT).unwrap().to_string(),
    ///     &format!("\"{FILENAME_EXTENSION_DEFAULT}\""));
    /// ```
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

        // Get the root dir.
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

        // Insert invironment.
        let mut context = Self {
            ct,
            path,
            dir_path,
            root_path,
        };
        context.insert_environment();
        context
    }

    /// Inserts the YAML front header variables in the context for later use
    /// with templates.
    ///
    pub(crate) fn insert_front_matter(&mut self, fm: &FrontMatter) {
        let mut tera_map = tera::Map::new();

        for (name, value) in fm.iter() {
            // Flatten all types.
            let val = match value {
                tera::Value::String(_) => value.to_owned(),
                tera::Value::Number(_) => value.to_owned(),
                tera::Value::Bool(_) => value.to_owned(),
                _ => tera::Value::String(value.to_string()),
            };

            // First we register a copy with the original variable name.
            tera_map.insert(name.to_string(), val.to_owned());

            // Here we register `fm_<var_name>`.
            let mut var_name = TMPL_VAR_FM_.to_string();
            var_name.push_str(name);
            self.ct.insert(&var_name, &val);
        }
        // Register the collection as `Object(Map<String, Value>)`.
        self.ct.insert(TMPL_VAR_FM_ALL, &tera_map);
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
    /// use tpnote_lib::context::Context;
    /// use tpnote_lib::content::Content;
    /// use tpnote_lib::content::ContentString;
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
    /// assert_eq!(&context.get("fm_title").unwrap().to_string(),
    ///     r#""My Stdin.""#);
    /// ```
    pub fn insert_content(
        &mut self,
        tmpl_var: &str,
        tmpl_var_header: &str,
        input: &impl Content,
    ) -> Result<(), NoteError> {
        // Register input .
        (*self).insert(tmpl_var_header, input.header());
        (*self).insert(tmpl_var, input.body());

        // Can we find a front matter in the input stream? If yes, the
        // unmodified input stream is our new note content.
        let input_fm = FrontMatter::try_from_content(input);
        match input_fm {
            Ok(ref fm) => {
                if fm.assert_not_empty().is_ok() {
                    log::trace!(
                        "YAML front matter in the input stream \"{}\" stdin found:\n{:#?}",
                        tmpl_var,
                        &fm
                    )
                }
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
        Ok(())
    }

    /// Captures _Tp-Note_'s environment and stores it as variables in a
    /// `context` collection. The variables are needed later to populate
    /// a context template and a filename template.
    ///
    /// This function add the keys:
    /// TMPL_VAR_EXTENSION_DEFAULT, TMPL_VAR_USERNAME and TMPL_VAR_LANG.
    ///
    /// ```
    /// use std::path::Path;
    /// use tpnote_lib::context::Context;
    /// use tpnote_lib::config::TMPL_VAR_EXTENSION_DEFAULT; // `extension_default`
    /// use tpnote_lib::config::FILENAME_EXTENSION_DEFAULT; // usually `md`
    /// let mut context = Context::from(&Path::new("/path/to/mynote.md"));
    ///
    /// // For most platforms `context.get("extension_default")` is `md`
    /// assert_eq!(&context.get(TMPL_VAR_EXTENSION_DEFAULT).unwrap().to_string(),
    ///     &format!("\"{FILENAME_EXTENSION_DEFAULT}\""));
    /// ```
    fn insert_environment(&mut self) {
        let lib_cfg = LIB_CFG.read().unwrap();
        let settings = SETTINGS.read().unwrap();

        // Default extension for new notes as defined in the configuration file.
        (*self).insert(
            TMPL_VAR_EXTENSION_DEFAULT,
            lib_cfg.filename.extension_default.as_str(),
        );

        // Search for UNIX, Windows and MacOS user-names.
        (*self).insert(TMPL_VAR_USERNAME, &settings.author);

        // Get the user's language tag.
        (*self).insert(TMPL_VAR_LANG, &settings.lang);
    }
}

/// Auto-dereference for convenient access to `tera::Context`.
impl Deref for Context {
    type Target = tera::Context;

    fn deref(&self) -> &Self::Target {
        &self.ct
    }
}

/// Auto-dereference for convenient access to `tera::Context`.
impl DerefMut for Context {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.ct
    }
}

#[cfg(test)]
mod tests {

    #[test]
    fn test_insert_front_matter() {
        use crate::context::Context;
        use crate::front_matter::FrontMatter;
        use std::path::Path;
        let mut context = Context::from(Path::new("/path/to/mynote.md"));
        context.insert_front_matter(&FrontMatter::try_from("title: \"My Stdin.\"").unwrap());

        assert_eq!(
            &context.get("fm_title").unwrap().to_string(),
            r#""My Stdin.""#
        );
    }
}
