//! Extends the built-in Tera filters.
use crate::content::Content;
use crate::error::NoteError;
use crate::note::FrontMatter;
use crate::note::TMPL_VAR_CLIPBOARD;
use crate::note::TMPL_VAR_CLIPBOARD_HEADER;
use crate::note::TMPL_VAR_DIR_PATH;
use crate::note::TMPL_VAR_EXTENSION_DEFAULT;
use crate::note::TMPL_VAR_FM_;
use crate::note::TMPL_VAR_FM_ALL;
use crate::note::TMPL_VAR_LANG;
use crate::note::TMPL_VAR_PATH;
use crate::note::TMPL_VAR_STDIN;
use crate::note::TMPL_VAR_STDIN_HEADER;
use crate::note::TMPL_VAR_USERNAME;
use crate::CFG;
use std::env;
use std::ops::Deref;
use std::ops::DerefMut;
use std::path::Path;
use std::path::PathBuf;
#[cfg(target_family = "windows")]
use windows_sys::Win32::Globalization::GetUserDefaultLocaleName;
#[cfg(target_family = "windows")]
use windows_sys::Win32::System::SystemServices::LOCALE_NAME_MAX_LENGTH;

/// Tiny wrapper around "Tera context" with some additional information.
#[derive(Debug, PartialEq)]
pub struct Context {
    /// Collection of substitution variables.
    ct: tera::Context,
    /// First positional command line argument.
    pub path: PathBuf,
    /// The directory (only) path corresponding to the first positional
    /// command line argument. The is our working directory and
    /// the directory where the note file is (will be) located.
    pub dir_path: PathBuf,
}

/// A thin wrapper around `tera::Context` storing some additional
/// information.
impl Context {
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

        // Register the canonicalized fully qualified file name.
        ct.insert(TMPL_VAR_PATH, &path);
        ct.insert(TMPL_VAR_DIR_PATH, &dir_path);

        Self { ct, path, dir_path }
    }

    /// Inserts the YAML front header variables in the context for later use with templates.
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

    /// Inserts clipboard and stdin data into the context. As these
    /// are `Content` structs, their header may carry also front matter
    /// variables. Those are added via `insert_front_matter()`.
    pub fn insert_content(
        &mut self,
        clipboard: &Content,
        stdin: &Content,
    ) -> Result<(), NoteError> {
        // Register input from clipboard.
        (*self).insert(
            TMPL_VAR_CLIPBOARD_HEADER,
            clipboard.borrow_dependent().header,
        );
        (*self).insert(TMPL_VAR_CLIPBOARD, clipboard.borrow_dependent().body);

        // Register input from stdin.
        (*self).insert(TMPL_VAR_STDIN_HEADER, stdin.borrow_dependent().header);
        (*self).insert(TMPL_VAR_STDIN, stdin.borrow_dependent().body);

        // Can we find a front matter in the input stream? If yes, the
        // unmodified input stream is our new note content.
        let stdin_fm = FrontMatter::try_from(&*stdin);
        match stdin_fm {
            Ok(ref stdin_fm) => log::trace!(
                "YAML front matter in the input stream stdin found:\n{:#?}",
                &stdin_fm
            ),
            Err(ref e) => {
                if !stdin.borrow_dependent().header.is_empty() {
                    return Err(NoteError::InvalidStdinYaml {
                        source_str: e.to_string(),
                    });
                }
            }
        };

        // Can we find a front matter in the clipboard? If yes, the unmodified
        // clipboard data is our new note content.
        let clipboard_fm = FrontMatter::try_from(&*clipboard);
        match clipboard_fm {
            Ok(ref clipboard_fm) => log::trace!(
                "YAML front matter in the clipboard found:\n{:#?}",
                &clipboard_fm
            ),
            Err(ref e) => {
                if !clipboard.borrow_dependent().header.is_empty() {
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
        Ok(())
    }

    /// Captures _Tp-Note_'s environment and stores it as variables in a
    /// `context` collection. The variables are needed later to populate
    /// a context template and a filename template.
    /// The `path` parameter must be a canonicalized fully qualified file name.
    pub fn insert_environment(&mut self) -> Result<(), NoteError> {
        // Default extension for new notes as defined in the configuration file.
        (*self).insert(
            TMPL_VAR_EXTENSION_DEFAULT,
            CFG.filename.extension_default.as_str(),
        );

        // Search for UNIX, Windows and MacOS user-names.
        let author = env::var("TPNOTEUSER").unwrap_or_else(|_| {
            env::var("LOGNAME").unwrap_or_else(|_| {
                env::var("USERNAME").unwrap_or_else(|_| env::var("USER").unwrap_or_default())
            })
        });
        (*self).insert(TMPL_VAR_USERNAME, &author);

        // Get the user's language tag.
        let tpnotelang = env::var("TPNOTELANG").ok();
        // Unix/MacOS version.
        #[cfg(not(target_family = "windows"))]
        if let Some(tpnotelang) = tpnotelang {
            (*self).insert(TMPL_VAR_LANG, &tpnotelang);
        } else {
            // [Linux: Define Locale and Language Settings - ShellHacks](https://www.shellhacks.com/linux-define-locale-language-settings/)
            let lang_env = env::var("LANG").unwrap_or_default();
            // [ISO 639](https://en.wikipedia.org/wiki/List_of_ISO_639-1_codes) language code.
            let mut language = "";
            // [ISO 3166](https://en.wikipedia.org/wiki/ISO_3166-1#Current_codes) country code.
            let mut territory = "";
            if let Some((l, lang_env)) = lang_env.split_once('_') {
                language = l;
                if let Some((t, _codeset)) = lang_env.split_once('.') {
                    territory = t;
                }
            }
            // [RFC 5646, Tags for the Identification of Languages](http://www.rfc-editor.org/rfc/rfc5646.txt)
            let mut lang = language.to_string();
            lang.push('-');
            lang.push_str(territory);
            (*self).insert(TMPL_VAR_LANG, &lang);
        }

        // Get the user's language tag.
        // Windows version.
        #[cfg(target_family = "windows")]
        if let Some(tpnotelang) = tpnotelang {
            (*self).insert(TMPL_VAR_LANG, &tpnotelang);
        } else {
            let mut lang = String::new();
            let mut buf = [0u16; LOCALE_NAME_MAX_LENGTH as usize];
            let len = unsafe { GetUserDefaultLocaleName(buf.as_mut_ptr(), buf.len() as i32) };
            if len > 0 {
                lang = String::from_utf16_lossy(&buf[..((len - 1) as usize)]);
            }
            (*self).insert(TMPL_VAR_LANG, &lang);
        }

        Ok(())
    }
}

/// Auto-dereference for convenient access to `tera::Content`.
impl Deref for Context {
    type Target = tera::Context;

    fn deref(&self) -> &Self::Target {
        &self.ct
    }
}

/// Auto-dereference for convenient access to `tera::Content`.
impl DerefMut for Context {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.ct
    }
}
