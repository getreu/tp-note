//! Extends the built-in Tera filters.
use tera::Value;

use crate::config::Assertion;
use crate::config::FILENAME_ROOT_PATH_MARKER;
use crate::config::LIB_CFG;
use crate::config::TMPL_HTML_VAR_DOC_ERROR;
use crate::config::TMPL_HTML_VAR_DOC_TEXT;
use crate::config::TMPL_HTML_VAR_EXPORTER_DOC_CSS;
use crate::config::TMPL_HTML_VAR_EXPORTER_HIGHLIGHTING_CSS;
use crate::config::TMPL_HTML_VAR_VIEWER_DOC_CSS_PATH;
use crate::config::TMPL_HTML_VAR_VIEWER_DOC_CSS_PATH_VALUE;
use crate::config::TMPL_HTML_VAR_VIEWER_DOC_JS;
use crate::config::TMPL_HTML_VAR_VIEWER_HIGHLIGHTING_CSS_PATH;
use crate::config::TMPL_HTML_VAR_VIEWER_HIGHLIGHTING_CSS_PATH_VALUE;
use crate::config::TMPL_VAR_CURRENT_SCHEME;
use crate::config::TMPL_VAR_DIR_PATH;
use crate::config::TMPL_VAR_DOC;
use crate::config::TMPL_VAR_DOC_FILE_DATE;
use crate::config::TMPL_VAR_DOC_HEADER;
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
use crate::error::FileError;
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
use std::fs::File;
use std::marker::PhantomData;
use std::matches;
use std::ops::Deref;
use std::path::Path;
use std::path::PathBuf;
use std::time::SystemTime;

/// At trait setting up a state machine as described below.
/// Its implementors represent one specific state defining the amount and the
/// type of data the `Context` type holds at that moment.
pub trait ContextState {}

#[derive(Debug, PartialEq, Clone)]
/// See description in the `ContextState` implementor list.
pub struct Invalid;

#[derive(Debug, PartialEq, Clone)]
/// See description in the `ContextState` implementor list.
pub struct HasSettings;

#[derive(Debug, PartialEq, Clone)]
/// See description in the `ContextState` implementor list.
pub struct ReadyForFilenameTemplate;

#[derive(Debug, PartialEq, Clone)]
/// See description in the `ContextState` implementor list.
pub struct HasExistingContent;

#[derive(Debug, PartialEq, Clone)]
/// See description in the `ContextState` implementor list.
pub struct ReadyForContentTemplate;

#[derive(Debug, PartialEq, Clone)]
/// See description in the `ContextState` implementor list.
pub struct ReadyForHtmlTemplate;

#[derive(Debug, PartialEq, Clone)]
/// See description in the `ContextState` implementor list.
pub struct ReadyForHtmlErrorTemplate;

/// The `Context` object is in an invalid state. Either it was not initialized
/// or its data does not correspond any more to the `Content` it represents.
///
/// |  State order   |                                       |
/// |----------------|---------------------------------------|
/// | Previous state | none                                  |
/// | Current state  | `Invalid`                             |
/// | Next state     | `HasSettings`                         |
///
impl ContextState for Invalid {}

/// The `Context` has the following initialized and valid fields: `path`,
/// `dir_path`, `root_path` and `ct`. The context `ct` contains data from
/// `insert_config_vars()` and `insert_settings()`.
/// `Context<HasSettings>` has the following variables set:
///
/// * `TMPL_VAR_CURRENT_SCHEME`
/// * `TMPL_VAR_DIR_PATH` in sync with `self.dir_path` and
/// * `TMPL_VAR_DOC_FILE_DATE` in sync with `self.doc_file_date` (only if
///   available).
/// * `TMPL_VAR_EXTENSION_DEFAULT`
/// * `TMPL_VAR_LANG`
/// * `TMPL_VAR_PATH` in sync with `self.path`,
/// * `TMPL_VAR_ROOT_PATH` in sync with `self.root_path`.
/// * `TMPL_VAR_SCHEME_SYNC_DEFAULT`.
/// * `TMPL_VAR_USERNAME`
///
/// The variables are inserted by the following methods: `self.from()`,
/// `self.insert_config_vars()` and `self.insert_settings()`.
/// Once this state is achieved, `Context` is constant and write protected until
/// the next state transition.
///
/// |  State order   |                                       |
/// |----------------|---------------------------------------|
/// | Previous state | `Invalid`                             |
/// | Current state  | `HasSettings`                         |
/// | Next state     | `ReadyForFilenameTemplate` or `HasExistingContent` |
///
impl ContextState for HasSettings {}

/// In addition to `HasSettings`, the `context.ct` contains template variables
/// deserialized from some note's front matter. E.g. a field named `title:`
/// appears in the context as `fm.fm_title` template variable.
/// In `Note` objects the `Content` is always associated with a
/// `Context<ReadyForFilenameTemplate>`.
/// Once this state is achieved, `Context` is constant and write protected until
/// the next state transition.
///
/// |  State order   |                                       |
/// |----------------|---------------------------------------|
/// | Previous state | `HasSettings`                         |
/// | Current state  | `ReadyForFilenameTemplate `           |
/// | Next state     | none or `ReadyForHtmlTemplate`        |
///
impl ContextState for ReadyForFilenameTemplate {}

/// In addition to the `HasSettings` the YAML headers of all clipboard
/// `Content` objects are registered as front matter variables `fm.fm*` in the
/// `Context`.
/// This stage is also used for the `TemplateKind::FromTextFile` template.
/// In this case the last inserted `Content` comes from the text file
/// the command line parameter `<path>` points to. This adds the following key:
///
/// * `TMPL_VAR_DOC`
///
/// This state can evolve as the
/// `insert_front_matter_and_raw_text_from_existing_content()` function can be
/// called several times.
///
/// |  State order   |                                       |
/// |----------------|---------------------------------------|
/// | Previous state | `HasSettings` or `HasExistingContent` |
/// | Current state  | `HasExistingContent`                  |
/// | Next state     | `ReadyForContentTemplate`             |
///
impl ContextState for HasExistingContent {}

/// This marker state means that enough information have been collected
/// in the `HasExistingContent` state to be passed to a
/// content template renderer.
/// Once this state is achieved, `Context` is constant and write protected until
/// the next state transition.
///
/// |  State order   |                                       |
/// |----------------|---------------------------------------|
/// | Previous state | `HasExistingContent`                  |
/// | Current state  | `ReadyForContentTemplate`             |
/// | Next state     | none                                  |
///
impl ContextState for ReadyForContentTemplate {}

/// In addition to the `ReadyForFilenameTemplate` state this state has the
/// following variables set:
///
/// * `TMPL_HTML_VAR_EXPORTER_DOC_CSS`
/// * `TMPL_HTML_VAR_EXPORTER_HIGHLIGHTING_CSS`
/// * `TMPL_HTML_VAR_EXPORTER_HIGHLIGHTING_CSS`
/// * `TMPL_HTML_VAR_VIEWER_DOC_CSS_PATH`
/// * `TMPL_HTML_VAR_VIEWER_DOC_CSS_PATH_VALUE`
/// * `TMPL_HTML_VAR_VIEWER_DOC_JS` from `viewer_doc_js`
/// * `TMPL_HTML_VAR_VIEWER_HIGHLIGHTING_CSS_PATH`
/// * `TMPL_HTML_VAR_VIEWER_HIGHLIGHTING_CSS_PATH_VALUE`
/// * `TMPL_VAR_DOC`
/// * `TMPL_VAR_DOC_HEADER`
///
/// Once this state is achieved, `Context` is constant and write protected until
/// the next state transition.
///
/// |  State order   |                                       |
/// |----------------|---------------------------------------|
/// | Previous state | `ReadyForFilenameTemplate`            |
/// | Current state  | `ReadyForHtmlTemplate`                |
/// | Next state     | none                                  |
///
impl ContextState for ReadyForHtmlTemplate {}

/// The `Context` has all data for the intended template.
///
/// * `TMPL_HTML_VAR_DOC_ERROR` from `error_message`
/// * `TMPL_HTML_VAR_DOC_TEXT` from `note_erroneous_content`
/// * `TMPL_HTML_VAR_VIEWER_DOC_JS` from `viewer_doc_js`
///
/// Once this state is achieved, `Context` is constant and write protected until
/// the next state transition.
///
/// |  State order   |                                       |
/// |----------------|---------------------------------------|
/// | Previous state | `HasSettings`                         |
/// | Current state  | `ReadyForHtmlErrorTemplate`           |
/// | Next state     | none                                  |
///
impl ContextState for ReadyForHtmlErrorTemplate {}

/// Tiny wrapper around "Tera context" with some additional information.
#[derive(Clone, Debug, PartialEq)]
pub struct Context<S: ContextState + ?Sized> {
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
    /// If `path` points to a file, we store its creation date here.
    pub doc_file_date: Option<SystemTime>,
    /// Rust requires usage of generic parameters, here `S`.
    _marker: PhantomData<S>,
}

/// The methods below are available in all `ContentState` states.
impl<S: ContextState> Context<S> {
    /// Getter for `self.path`.
    pub fn get_path(&self) -> &Path {
        self.path.as_path()
    }

    /// Getter for `self.dir_path`.
    pub fn get_dir_path(&self) -> &Path {
        self.dir_path.as_path()
    }

    /// Getter for `self.root_path`.
    pub fn get_root_path(&self) -> &Path {
        self.root_path.as_path()
    }

    /// Getter for `self.doc_file_date`.
    pub fn get_doc_file_date(&self) -> Option<SystemTime> {
        self.doc_file_date
    }

    /// Transition to the fault state.
    pub fn mark_as_invalid(self) -> Context<Invalid> {
        Context {
            ct: self.ct,
            path: self.path,
            dir_path: self.dir_path,
            root_path: self.root_path,
            doc_file_date: self.doc_file_date,
            _marker: PhantomData,
        }
    }

    /// Constructor. Unlike `from()` this constructor does not access
    /// the file system in order to detect `dir_path`, `root_path` and
    /// `doc_file_date`. It copies these values from the passed `context`.
    /// Use this constructor when you are sure that the above date has
    /// not changed since you instantiated `context`. In this case you
    /// can avoid repeated file access.
    pub fn from_context_path(context: &Context<S>) -> Context<HasSettings> {
        let mut new_context = Context {
            ct: tera::Context::new(),
            path: context.path.clone(),
            dir_path: context.dir_path.clone(),
            root_path: context.root_path.clone(),
            doc_file_date: context.doc_file_date,
            _marker: PhantomData,
        };

        new_context.sync_paths_to_map();
        new_context.insert_config_vars();
        new_context.insert_settings();
        new_context
    }

    /// Helper function that keeps the values with the `self.ct` key
    ///
    /// * `TMPL_VAR_PATH` in sync with `self.path`,
    /// * `TMPL_VAR_DIR_PATH` in sync with `self.dir_path` and
    /// * `TMPL_VAR_ROOT_PATH` in sync with `self.root_path`.
    /// * `TMPL_VAR_DOC_FILE_DATE` in sync with `self.doc_file_date` (only if
    ///
    /// available).
    /// Synchronization is performed by copying the latter to the former.
    fn sync_paths_to_map(&mut self) {
        self.ct.insert(TMPL_VAR_PATH, &self.path);
        self.ct.insert(TMPL_VAR_DIR_PATH, &self.dir_path);
        self.ct.insert(TMPL_VAR_ROOT_PATH, &self.root_path);
        if let Some(time) = self.doc_file_date {
            self.ct.insert(
                TMPL_VAR_DOC_FILE_DATE,
                &time
                    .duration_since(SystemTime::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs(),
            )
        } else {
            self.ct.remove(TMPL_VAR_DOC_FILE_DATE);
        };
    }

    /// Helper function that asserts;
    ///
    /// * `TMPL_VAR_PATH` in sync with `self.path`,
    /// * `TMPL_VAR_DIR_PATH` in sync with `self.dir_path` and
    /// * `TMPL_VAR_ROOT_PATH` in sync with `self.root_path`.
    /// * `TMPL_VAR_DOC_FILE_DATE` in sync with `self.doc_file_date` (only if
    ///   available).
    ///
    /// This data is intentionally redundant, this is why we check if it is
    /// still in sync.
    pub(crate) fn debug_assert_paths_and_map_in_sync(&self) {
        debug_assert_eq!(
            self.ct.get(TMPL_VAR_PATH).unwrap().as_str(),
            self.path.to_str()
        );
        debug_assert_eq!(
            self.ct.get(TMPL_VAR_DIR_PATH).unwrap().as_str(),
            self.dir_path.to_str()
        );
        debug_assert_eq!(
            self.ct.get(TMPL_VAR_ROOT_PATH).unwrap().as_str(),
            self.root_path.to_str()
        );
        debug_assert_eq!(
            if let Some(val) = self.ct.get(TMPL_VAR_DOC_FILE_DATE) {
                val.as_number().unwrap().as_u64().unwrap()
            } else {
                0
            },
            if let Some(st) = self.doc_file_date {
                st.duration_since(SystemTime::UNIX_EPOCH).unwrap().as_secs()
            } else {
                0
            }
        );
    }

    /// Insert some configuration variables into the context so that they
    /// can be used in the templates.
    ///
    /// This function adds the key:
    ///
    /// * `TMPL_VAR_SCHEME_SYNC_DEFAULT`.
    ///
    /// ```
    /// use std::path::Path;
    /// use tpnote_lib::config::TMPL_VAR_SCHEME_SYNC_DEFAULT;
    /// use tpnote_lib::settings::set_test_default_settings;
    /// use tpnote_lib::context::Context;
    /// set_test_default_settings().unwrap();
    ///
    /// // The constructor calls `context.insert_settings()` before returning.
    /// let mut context = Context::from(&Path::new("/path/to/mynote.md")).unwrap();
    ///
    /// // When the note's YAML header does not contain a `scheme:` field,
    /// // the `default` scheme is used.
    /// assert_eq!(&context.get(TMPL_VAR_SCHEME_SYNC_DEFAULT).unwrap().to_string(),
    ///     &format!("\"default\""));
    /// ```
    fn insert_config_vars(&mut self) {
        let lib_cfg = LIB_CFG.read_recursive();

        // Default extension for new notes as defined in the configuration file.
        self.ct.insert(
            TMPL_VAR_SCHEME_SYNC_DEFAULT,
            lib_cfg.scheme_sync_default.as_str(),
        );
    }

    /// Captures Tp-Note's environment and stores it as variables in a
    /// `context` collection. The variables are needed later to populate
    /// a context template and a filename template.
    ///
    /// This function adds the keys:
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
    /// let mut context = Context::from(&Path::new("/path/to/mynote.md")).unwrap();
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
        self.ct.insert(
            TMPL_VAR_EXTENSION_DEFAULT,
            settings.extension_default.as_str(),
        );

        {
            let lib_cfg = LIB_CFG.read_recursive();
            self.ct.insert(
                TMPL_VAR_CURRENT_SCHEME,
                &lib_cfg.scheme[settings.current_scheme].name,
            );
        } // Release `lib_cfg` here.

        // Search for UNIX, Windows and MacOS user-names.
        self.ct.insert(TMPL_VAR_USERNAME, &settings.author);

        // Get the user's language tag.
        self.ct.insert(TMPL_VAR_LANG, &settings.lang);
    }

    /// Inserts the YAML front header variables into the context for later use
    /// with templates.
    ///
    fn insert_front_matter2(&mut self, fm: &FrontMatter) {
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

    /// Insert a key/val pair directly. Only available in tests.
    #[cfg(test)]
    pub(crate) fn insert(&mut self, key: &str, val: &tera::Value) {
        self.ct.insert(key, val);
    }
}

/// The start state of all `Context` objects.
///
impl Context<Invalid> {
    /// Constructor: `path` is the first positional command line parameter
    /// `<path>` (see man page). `path` must point to a directory or
    /// a file.
    ///
    /// A copy of `path` is stored in `self.ct` as key `TMPL_VAR_PATH`. It
    /// directory path as key `TMPL_VAR_DIR_PATH`. The root directory, where
    /// the marker file `tpnote.toml` was found, is stored with the key
    /// `TMPL_VAR_ROOT_PATH`. If `path` points to a file, its file creation
    /// date is stored with the key `TMPL_VAR_DOC_FILE_DATE`.
    ///
    /// ```rust
    /// use std::path::Path;
    /// use tpnote_lib::settings::set_test_default_settings;
    /// use tpnote_lib::config::TMPL_VAR_DIR_PATH;
    /// use tpnote_lib::config::TMPL_VAR_PATH;
    /// use tpnote_lib::context::Context;
    /// set_test_default_settings().unwrap();
    ///
    /// let mut context = Context::from(&Path::new("/path/to/mynote.md")).unwrap();
    ///
    /// assert_eq!(context.path, Path::new("/path/to/mynote.md"));
    /// assert_eq!(context.dir_path, Path::new("/path/to/"));
    /// assert_eq!(&context.get(TMPL_VAR_PATH).unwrap().to_string(),
    ///             r#""/path/to/mynote.md""#);
    /// assert_eq!(&context.get(TMPL_VAR_DIR_PATH).unwrap().to_string(),
    ///             r#""/path/to""#);
    /// ```
    pub fn from(path: &Path) -> Result<Context<HasSettings>, FileError> {
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

        // Get the file's creation date. Fail silently.
        let file_creation_date = if let Ok(file) = File::open(&path) {
            let metadata = file.metadata()?;
            if let Ok(time) = metadata.created().or_else(|_| metadata.modified()) {
                Some(time)
            } else {
                None
            }
        } else {
            None
        };

        // Insert environment.
        let mut context = Context {
            ct: tera::Context::new(),
            path,
            dir_path,
            root_path,
            doc_file_date: file_creation_date,
            _marker: PhantomData,
        };

        context.sync_paths_to_map();
        context.insert_config_vars();
        context.insert_settings();
        Ok(context)
    }
}

impl Context<HasSettings> {
    /// Merges `fm` into `self.ct`.
    pub fn insert_front_matter(mut self, fm: &FrontMatter) -> Context<ReadyForFilenameTemplate> {
        Context::insert_front_matter2(&mut self, fm);
        Context {
            ct: self.ct,
            path: self.path,
            dir_path: self.dir_path,
            root_path: self.root_path,
            doc_file_date: self.doc_file_date,
            _marker: PhantomData,
        }
    }

    /// Inserts clipboard data, stdin data and/or existing note file content
    /// into the context. The data may contain some copied text with or without
    /// a YAML header. The latter usually carries front matter variables.
    /// The `input` data below is registered with the key name given by
    /// `tmpl_var_body_name`. Typical names are `"clipboard"` or `"stdin"`. If
    /// the below `input` contains a valid YAML header, it will be registered
    /// in the context with the key name given by `tmpl_var_header_name`. The
    /// templates expect the key names `clipboard_header` or `std_header`. The
    /// raw header text will be inserted with this key name.
    ///
    /// ```rust
    /// use std::path::Path;
    /// use tpnote_lib::settings::set_test_default_settings;
    /// use tpnote_lib::context::Context;
    /// use tpnote_lib::content::Content;
    /// use tpnote_lib::content::ContentString;
    /// set_test_default_settings().unwrap();
    ///
    /// let mut context = Context::from(&Path::new("/path/to/mynote.md")).unwrap();
    /// let c1 =  ContentString::from_string(String::from("Data from clipboard."),
    ///          "txt_clipboard_header".to_string(),
    ///          "txt_clipboard".to_string(),
    /// );
    /// let c2 = ContentString::from_string(
    ///          "---\ntitle: My Stdin.\n---\nbody".to_string(),
    ///          "stdin_header".to_string(),
    ///          "stdin".to_string(),
    /// );
    /// let c = vec![&c1, &c2];
    ///
    /// let context = context
    ///     .insert_front_matter_and_raw_text_from_existing_content(&c).unwrap();
    ///
    /// assert_eq!(&context.get("txt_clipboard").unwrap().to_string(),
    ///     "\"Data from clipboard.\"");
    /// assert_eq!(&context.get("stdin").unwrap().to_string(),
    ///     "\"body\"");
    /// assert_eq!(&context.get("stdin_header").unwrap().to_string(),
    ///     "\"title: My Stdin.\"");
    /// // "fm_title" is dynamically generated from the header variable "title".
    /// assert_eq!(&context
    ///            .get("fm").unwrap()
    ///            .get("fm_title").unwrap().to_string(),
    ///      "\"My Stdin.\"");
    /// ```
    pub fn insert_front_matter_and_raw_text_from_existing_content(
        self,
        clipboards: &Vec<&impl Content>,
    ) -> Result<Context<HasExistingContent>, NoteError> {
        let context: Context<HasExistingContent> = Context {
            ct: self.ct,
            path: self.path,
            dir_path: self.dir_path,
            root_path: self.root_path,
            doc_file_date: self.doc_file_date,
            _marker: PhantomData,
        };

        let context = context.insert_front_matter_and_raw_text_from_existing_content(clipboards)?;
        Ok(context)
    }

    /// This adds the following variables to `self`:
    ///
    /// * `TMPL_HTML_VAR_VIEWER_DOC_JS` from `viewer_doc_js`
    /// * `TMPL_HTML_VAR_DOC_ERROR` from `error_message`
    /// * `TMPL_HTML_VAR_DOC_TEXT` from `note_erroneous_content`
    ///
    pub fn insert_error_content(
        mut self,
        note_erroneous_content: &impl Content,
        error_message: &str,
        // Java Script live updater inject code. Will be inserted into
        // `tmpl_html.viewer`.
        viewer_doc_js: &str,
    ) -> Context<ReadyForHtmlErrorTemplate> {
        //
        self.ct.insert(TMPL_HTML_VAR_VIEWER_DOC_JS, viewer_doc_js);

        self.ct.insert(TMPL_HTML_VAR_DOC_ERROR, error_message);
        self.ct
            .insert(TMPL_HTML_VAR_DOC_TEXT, &note_erroneous_content.as_str());

        Context {
            ct: self.ct,
            path: self.path,
            dir_path: self.dir_path,
            root_path: self.root_path,
            doc_file_date: self.doc_file_date,
            _marker: PhantomData,
        }
    }
}

impl Context<HasExistingContent> {
    /// See function of the same name in `impl Context<HasSettings>`.
    pub fn insert_front_matter_and_raw_text_from_existing_content(
        mut self,
        clipboards: &Vec<&impl Content>,
    ) -> Result<Context<HasExistingContent>, NoteError> {
        //
        for clip in clipboards {
            // Register input.
            self.ct.insert(clip.header_name(), clip.header());
            self.ct.insert(clip.body_name(), clip.body());

            // Can we find a front matter in the input stream? If yes, the
            // unmodified input stream is our new note content.
            if !clip.header().is_empty() {
                let input_fm = FrontMatter::try_from(clip.header());
                match input_fm {
                    Ok(ref fm) => {
                        log::trace!(
                            "Input stream from \"{}\" generates the front matter variables:\n{:#?}",
                            clip.body(),
                            &fm
                        )
                    }
                    Err(ref e) => {
                        if !clip.header().is_empty() {
                            return Err(NoteError::InvalidInputYaml {
                                tmpl_var: clip.body_name().to_string(),
                                source_str: e.to_string(),
                            });
                        }
                    }
                };

                // Register front matter.
                // The variables registered here can be overwrite the ones from the clipboard.
                if let Ok(fm) = input_fm {
                    self.insert_front_matter2(&fm);
                }
            }
        }
        Ok(self)
    }

    /// Mark this as ready for a content template.
    pub fn set_state_ready_for_content_template(self) -> Context<ReadyForContentTemplate> {
        self.debug_assert_paths_and_map_in_sync();
        Context {
            ct: self.ct,
            path: self.path,
            dir_path: self.dir_path,
            root_path: self.root_path,
            doc_file_date: self.doc_file_date,
            _marker: PhantomData,
        }
    }
}

impl Context<ReadyForFilenameTemplate> {
    /// Checks if the front matter variables satisfy preconditions.
    /// `self.path` is the path to the current document.
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

    /// Indicates that this context contains all we need for the content
    /// template.
    #[cfg(test)]
    pub(crate) fn set_state_ready_for_content_template(self) -> Context<ReadyForContentTemplate> {
        self.debug_assert_paths_and_map_in_sync();
        Context {
            ct: self.ct,
            path: self.path,
            dir_path: self.dir_path,
            root_path: self.root_path,
            doc_file_date: self.doc_file_date,
            _marker: PhantomData,
        }
    }

    /// Inserts the following variables into `self`:
    ///
    /// * `TMPL_HTML_VAR_VIEWER_DOC_JS` from `viewer_doc_js`
    /// * `TMPL_VAR_DOC_HEADER` from `content.header()`
    /// * `TMPL_VAR_DOC` from `content.body()`
    /// * `TMPL_HTML_VAR_EXPORTER_DOC_CSS`
    /// * `TMPL_HTML_VAR_EXPORTER_HIGHLIGHTING_CSS`
    /// * `TMPL_HTML_VAR_EXPORTER_HIGHLIGHTING_CSS`
    /// * `TMPL_HTML_VAR_VIEWER_DOC_CSS_PATH`
    /// * `TMPL_HTML_VAR_VIEWER_DOC_CSS_PATH_VALUE`
    /// * `TMPL_HTML_VAR_VIEWER_HIGHLIGHTING_CSS_PATH`
    /// * `TMPL_HTML_VAR_VIEWER_HIGHLIGHTING_CSS_PATH_VALUE`
    ///
    pub fn insert_raw_content_and_css(
        mut self,
        content: &impl Content,
        viewer_doc_js: &str,
    ) -> Context<ReadyForHtmlTemplate> {
        //
        self.ct.insert(TMPL_HTML_VAR_VIEWER_DOC_JS, viewer_doc_js);

        self.ct.insert(TMPL_VAR_DOC_HEADER, content.header());
        self.ct.insert(TMPL_VAR_DOC, content.body());

        {
            let lib_cfg = &LIB_CFG.read_recursive();

            // Insert the raw CSS
            self.ct.insert(
                TMPL_HTML_VAR_EXPORTER_DOC_CSS,
                &(lib_cfg.tmpl_html.exporter_doc_css),
            );

            // Insert the raw CSS
            #[cfg(feature = "renderer")]
            self.ct.insert(
                TMPL_HTML_VAR_EXPORTER_HIGHLIGHTING_CSS,
                &(lib_cfg.tmpl_html.exporter_highlighting_css),
            );
        } // Drop `lib_cfg`.

        // Insert the raw CSS
        #[cfg(not(feature = "renderer"))]
        self.ct.insert(TMPL_HTML_VAR_EXPORTER_HIGHLIGHTING_CSS, "");

        // Insert the web server path to get the Tp-Note's CSS loaded.
        self.ct.insert(
            TMPL_HTML_VAR_VIEWER_DOC_CSS_PATH,
            TMPL_HTML_VAR_VIEWER_DOC_CSS_PATH_VALUE,
        );

        // Insert the web server path to get the highlighting CSS loaded.
        self.ct.insert(
            TMPL_HTML_VAR_VIEWER_HIGHLIGHTING_CSS_PATH,
            TMPL_HTML_VAR_VIEWER_HIGHLIGHTING_CSS_PATH_VALUE,
        );
        Context {
            ct: self.ct,
            path: self.path,
            dir_path: self.dir_path,
            root_path: self.root_path,
            doc_file_date: self.doc_file_date,
            _marker: PhantomData,
        }
    }
}

/// Auto dereferences for convenient access to `tera::Context`.
impl<S: ContextState> Deref for Context<S> {
    type Target = tera::Context;

    fn deref(&self) -> &Self::Target {
        &self.ct
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
        let context = Context::from(Path::new("/path/to/mynote.md")).unwrap();
        let context = context
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
        let context = Context::from(Path::new("/path/to/mynote.md")).unwrap();
        let context = context
            .insert_front_matter(&FrontMatter::try_from("title: My Stdin.\nsome: text").unwrap());
        let context = context.set_state_ready_for_content_template();

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
        let cx = Context::from(Path::new("does not matter")).unwrap();
        let cx = cx.insert_front_matter(&fm);

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
        let cx = Context::from(Path::new("./03b-test.md")).unwrap();
        let cx = cx.insert_front_matter(&fm);

        assert!(matches!(cx.assert_precoditions(), Ok(())));

        //
        // Should not be a compound type.
        let input = "# document start
        title: The book
        sort_tag:
        -    1234
        -    456";
        let fm = FrontMatter::try_from(input).unwrap();
        let cx = Context::from(Path::new("does not matter")).unwrap();
        let cx = cx.insert_front_matter(&fm);

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
        let cx = Context::from(Path::new("does not matter")).unwrap();
        let cx = cx.insert_front_matter(&fm);

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
        let cx = Context::from(Path::new("does not matter")).unwrap();
        let cx = cx.insert_front_matter(&fm);

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
        let cx = Context::from(Path::new("does not matter")).unwrap();
        let cx = cx.insert_front_matter(&fm);

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
        let cx = Context::from(Path::new("does not matter")).unwrap();
        let cx = cx.insert_front_matter(&fm);
        assert_eq!(cx.get(TMPL_VAR_FM_ALL).unwrap(), &expected);

        //
        let input = "# document start
        title: my title
        file_ext: ''
        ";
        let expected = json!({"fm_title": "my title", "fm_file_ext": ""});

        let fm = FrontMatter::try_from(input).unwrap();
        let cx = Context::from(Path::new("does not matter")).unwrap();
        let cx = cx.insert_front_matter(&fm);
        assert_eq!(cx.get(TMPL_VAR_FM_ALL).unwrap(), &expected);

        //
        let input = "# document start
        title: ''
        subtitle: my subtitle
        ";
        let fm = FrontMatter::try_from(input).unwrap();
        let cx = Context::from(Path::new("does not matter")).unwrap();
        let cx = cx.insert_front_matter(&fm);

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
        let cx = Context::from(Path::new("does not matter")).unwrap();
        let cx = cx.insert_front_matter(&fm);

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
        let cx = Context::from(Path::new("does not matter")).unwrap();
        let cx = cx.insert_front_matter(&fm);

        assert!(matches!(
            cx.assert_precoditions().unwrap_err(),
            NoteError::FrontMatterFieldIsNotString { .. }
        ));
    }
}
