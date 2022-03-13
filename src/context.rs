//! Extends the built-in Tera filters.
use crate::error::NoteError;
use crate::note::FrontMatter;
use crate::note::TMPL_VAR_CLIPBOARD;
use crate::note::TMPL_VAR_CLIPBOARD_HEADER;
use crate::note::TMPL_VAR_DIR_PATH;
use crate::note::TMPL_VAR_EXTENSION_DEFAULT;
use crate::note::TMPL_VAR_FM_;
use crate::note::TMPL_VAR_FM_ALL;
use crate::note::TMPL_VAR_PATH;
use crate::note::TMPL_VAR_STDIN;
use crate::note::TMPL_VAR_STDIN_HEADER;
use crate::note::TMPL_VAR_USERNAME;
use crate::settings::CLIPBOARD;
use crate::settings::STDIN;
use crate::CFG;
use std::env;
use std::ops::Deref;
use std::ops::DerefMut;
use std::path::Path;
use std::path::PathBuf;

/// Tiny wrapper around "Tera context" with some additional information.
#[derive(Debug, PartialEq)]
pub struct ContextWrapper {
    // Collection of substitution variables.
    ct: tera::Context,
    // The note's directory path on disk.
    pub dir_path: PathBuf,
}

/// A thin wrapper around `tera::Context` storing some additional
/// information.
impl ContextWrapper {
    pub fn new() -> Self {
        Self {
            ct: tera::Context::new(),
            dir_path: PathBuf::new(),
        }
    }

    /// Inserts the YAML front header variable in the context for later use with templates.
    /// We register only flat `tera::Value` types.
    /// If there is a list, concatenate its items with `, ` and register the result
    /// as a flat string.
    pub fn insert_front_matter(&mut self, fm: &FrontMatter) {
        let mut tera_map = tera::Map::new();

        for (name, value) in &fm.map {
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

    /// Captures _Tp-Note_'s environment and stores it as variables in a
    /// `context` collection. The variables are needed later to populate
    /// a context template and a filename template.
    /// The `path` parameter must be a canonicalized fully qualified file name.
    pub fn insert_environment(&mut self, path: &Path) -> Result<(), NoteError> {
        // Register the canonicalized fully qualified file name.
        let file = path.to_str().unwrap_or_default();
        (*self).insert(TMPL_VAR_PATH, &file);

        // `dir_path` is a directory as fully qualified path, ending
        // by a separator.
        let dir_path = if path.is_dir() {
            path
        } else {
            path.parent().unwrap_or_else(|| Path::new("./"))
        };
        (*self).insert(TMPL_VAR_DIR_PATH, &dir_path.to_str().unwrap_or_default());

        // Register input from clipboard.
        (*self).insert(
            TMPL_VAR_CLIPBOARD_HEADER,
            CLIPBOARD.borrow_dependent().header,
        );
        (*self).insert(TMPL_VAR_CLIPBOARD, CLIPBOARD.borrow_dependent().body);

        // Register input from stdin.
        (*self).insert(TMPL_VAR_STDIN_HEADER, STDIN.borrow_dependent().header);
        (*self).insert(TMPL_VAR_STDIN, STDIN.borrow_dependent().body);

        // Can we find a front matter in the input stream? If yes, the
        // unmodified input stream is our new note content.
        let stdin_fm = FrontMatter::try_from(&*STDIN);
        match stdin_fm {
            Ok(ref stdin_fm) => log::trace!(
                "YAML front matter in the input stream stdin found:\n{:#?}",
                &stdin_fm
            ),
            Err(ref e) => {
                if !STDIN.borrow_dependent().header.is_empty() {
                    return Err(NoteError::InvalidStdinYaml {
                        source_str: e.to_string(),
                    });
                }
            }
        };

        // Can we find a front matter in the clipboard? If yes, the unmodified
        // clipboard data is our new note content.
        let clipboard_fm = FrontMatter::try_from(&*CLIPBOARD);
        match clipboard_fm {
            Ok(ref clipboard_fm) => log::trace!(
                "YAML front matter in the clipboard found:\n{:#?}",
                &clipboard_fm
            ),
            Err(ref e) => {
                if !CLIPBOARD.borrow_dependent().header.is_empty() {
                    return Err(NoteError::InvalidClipboardYaml {
                        source_str: e.to_string(),
                    });
                }
            }
        };

        // Register clipboard front matter.
        if let Ok(fm) = clipboard_fm {
            self.insert_front_matter(&fm);
        }

        // Register stdin front matter.
        // The variables registered here can be overwrite the ones from the clipboard.
        if let Ok(fm) = stdin_fm {
            self.insert_front_matter(&fm);
        }

        // Default extension for new notes as defined in the configuration file.
        (*self).insert(
            TMPL_VAR_EXTENSION_DEFAULT,
            CFG.filename.extension_default.as_str(),
        );

        // search for UNIX, Windows and MacOS user-names
        let author = env::var("TPNOTEUSER").unwrap_or_else(|_| {
            env::var("LOGNAME").unwrap_or_else(|_| {
                env::var("USERNAME").unwrap_or_else(|_| env::var("USER").unwrap_or_default())
            })
        });
        (*self).insert(TMPL_VAR_USERNAME, &author);

        self.dir_path = dir_path.to_path_buf();

        Ok(())
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
