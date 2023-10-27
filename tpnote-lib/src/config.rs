//! Set configuration defaults, reads and writes _Tp-Note_'s configuration file
//! and exposes the configuration as the `static` variable `LIB_CFG` behind a
//! mutex. This makes it possible to modify all configuration defaults
//! (and templates) at runtime.
//!
//! ```rust
//! use tpnote_lib::config::LIB_CFG;
//!
//! let mut lib_cfg = LIB_CFG.write();
//! (*lib_cfg).filename.copy_counter_extra_separator = '@'.to_string();
//! ```
//!
//! Contract: although `LIB_CFG` is mutable at runtime, it is sourced only
//! once at the start of Tp-Note. All modification terminates before accessing
//! the high-level API in the `workflow` module of this crate.
#[cfg(feature = "renderer")]
use crate::highlight::get_viewer_highlighting_css;
use crate::{error::LibCfgError, markup_language::MarkupLanguage};
use lazy_static::lazy_static;
use parking_lot::RwLock;
use sanitize_filename_reader_friendly::TRIM_LINE_CHARS;
use serde::{Deserialize, Serialize};
use std::fmt::Write;
use std::str::FromStr;
#[cfg(feature = "renderer")]
use syntect::highlighting::ThemeSet;

/// Default library configuration as TOML.
pub const LIB_CONFIG_DEFAULT_TOML: &str = include_str!("config_default.toml");

/// Maximum length of a note's filename in bytes. If a filename template produces
/// a longer string, it will be truncated.
pub const FILENAME_LEN_MAX: usize =
    // Most file system's limit.
    255
    // Additional separator.
    - 2
    // Additional copy counter.
    - 5
    // Extra spare bytes, in case the user's copy counter is longer.
    - 6;

/// The appearance of a file with this filename marks the position of
/// `TMPL_VAR_ROOT_PATH`.
pub const FILENAME_ROOT_PATH_MARKER: &str = ".tpnote.toml";

/// When a filename is taken already, Tp-Note adds a copy
/// counter number in the range of `0..COPY_COUNTER_MAX`
/// at the end.
pub const FILENAME_COPY_COUNTER_MAX: usize = 400;

/// A filename extension, if prensent, is separated by a dot.
pub(crate) const FILENAME_EXTENSION_SEPARATOR_DOT: char = '.';

/// A dotfile starts with a dot.
pub(crate) const FILENAME_DOTFILE_MARKER: char = '.';

/// A sort-tag is allowed to contain letters, but only this
/// number in a row.
pub const FILENAME_SORT_TAG_LETTERS_IN_SUCCESSION_MAX: u8 = 2;

/// The template variable contains the fully qualified path of the `<path>`
/// command line argument. If `<path>` points to a file, the variable contains
/// the file path. If it points to a directory, it contains the directory path,
/// or - if no `path` is given - the current working directory.
pub const TMPL_VAR_PATH: &str = "path";

/// Contains the fully qualified directory path of the `<path>` command line
/// argument.
/// If `<path>` points to a file, the last component (the file name) is omitted.
/// If it points to a directory, the content of this variable is identical to
/// `TMPL_VAR_PATH`,
pub const TMPL_VAR_DIR_PATH: &str = "dir_path";

/// The root directory of the current note. This is the first directory,
/// that upwards from `TMPL_VAR_DIR_PATH`, contains a file named
/// `FILENAME_ROOT_PATH_MARKER`. The root directory is used by Tp-Note's viewer
/// as base directory
pub const TMPL_VAR_ROOT_PATH: &str = "root_path";

/// Contains the YAML header (if any) of the clipboard content.
/// Otherwise the empty string.
pub const TMPL_VAR_CLIPBOARD_HEADER: &str = "clipboard_header";

/// If there is a YAML header in the clipboard content, this contains
/// the body only. Otherwise, it contains the whole clipboard content.
pub const TMPL_VAR_CLIPBOARD: &str = "clipboard";

/// Contains the YAML header (if any) of the `stdin` input stream.
/// Otherwise the empty string.
pub const TMPL_VAR_STDIN_HEADER: &str = "stdin_header";

/// If there is a YAML header in the `stdin` input stream, this contains the
/// body only. Otherwise, it contains the whole input stream.
pub const TMPL_VAR_STDIN: &str = "stdin";

/// Contains the default file extension for new note files as defined in the
/// configuration file.
pub const TMPL_VAR_EXTENSION_DEFAULT: &str = "extension_default";

/// Contains the content of the first non empty environment variable
/// `LOGNAME`, `USERNAME` or `USER`.
pub const TMPL_VAR_USERNAME: &str = "username";

/// Contains the user's language tag as defined in
/// [RFC 5646](http://www.rfc-editor.org/rfc/rfc5646.txt).
/// Not to be confused with the UNIX `LANG` environment variable from which
/// this value is derived under Linux/MacOS.
/// Under Windows, the user's language tag is queried through the Win-API.
/// If defined, the environment variable `TPNOTE_LANG` overwrites this value
/// (all operating systems).
pub const TMPL_VAR_LANG: &str = "lang";
/// All the front matter fields serialized as text, exactly as they appear in
/// the front matter.
pub const TMPL_VAR_DOC_FM_TEXT: &str = "doc_fm_text";

/// Contains the body of the file the command line option `<path>`
/// points to. Only available in the `tmpl.from_text_file_content`,
/// `tmpl.sync_filename` and HTML templates.
pub const TMPL_VAR_DOC_BODY_TEXT: &str = "doc_body_text";

/// Contains the date of the file the command line option `<path>` points to.
/// The date is represented as an integer the way `std::time::SystemTime`
/// resolves to on the platform. Only available in the
/// `tmpl.from_text_file_content`, `tmpl.sync_filename` and HTML templates.
pub const TMPL_VAR_DOC_FILE_DATE: &str = "doc_file_date";

/// Prefix prepended to front matter field names when a template variable
/// is generated with the same name.
pub const TMPL_VAR_FM_: &str = "fm_";

/// Contains a Hash Map with all front matter fields. Lists are flattened
/// into strings. These variables are only available in the
/// `tmpl.from_text_file_content`, `tmpl.sync_filename` and HTML templates.
pub const TMPL_VAR_FM_ALL: &str = "fm_all";

/// By default, the template `tmpl.sync_filename` defines the function of
/// of this variable as follows:
/// Contains the value of the front matter field `file_ext` and determines the
/// markup language used to render the document. When the field is missing the
/// markup language is derived from the note's filename extension.
///
/// This is a dynamically generated variable originating from the front matter
/// of the current note. As all front matter variables, its value is copied as
/// it is without modification.  Here, the only special treatment is, when
/// analyzing the front matter, it is verified, that the value of this variable
/// is registered in one of the `filename.extensions_*` variables.
pub const TMPL_VAR_FM_FILE_EXT: &str = "fm_file_ext";

/// By default, the template `tmpl.sync_filename` defines the function of
/// of this variable as follows:
/// If this variable is defined, the _sort tag_ of the filename is replaced with
/// the value of this variable next time the filename is synchronized.  If not
/// defined, the sort tag of the filename is never changed.
///
/// This is a dynamically generated variable originating from the front matter
/// of the current note. As all front matter variables, its value is copied as
/// it is without modification.  Here, the only special treatment is, when
/// analyzing the front matter, it is verified, that all the characters of the
/// value of this variable are listed in `filename.sort_tag_chars`.
pub const TMPL_VAR_FM_SORT_TAG: &str = "fm_sort_tag";

/// Contains the value of the front matter field `no_filename_sync`.  When set
/// to `no_filename_sync:` or `no_filename_sync: true`, the filename
/// synchronisation mechanism is disabled for this note file.  Depreciated
/// in favour of `TMPL_VAR_FM_FILENAME_SYNC`.
pub const TMPL_VAR_FM_NO_FILENAME_SYNC: &str = "fm_no_filename_sync";

/// Contains the value of the front matter field `filename_sync`.  When set to
/// `filename_sync: false`, the filename synchronization mechanism is
/// disabled for this note file. Default value is `true`.
pub const TMPL_VAR_FM_FILENAME_SYNC: &str = "fm_filename_sync";

/// A pseudo language tag for the `get_lang_filter`. When placed in the
/// `TMP_FILTER_GET_LANG` list, all available languages are selected.
pub const TMPL_FILTER_GET_LANG_ALL: &str = "+all";

/// HTML template variable containing the automatically generated JavaScript
/// code to be included in the HTML rendition.
pub const TMPL_HTML_VAR_VIEWER_DOC_JS: &str = "viewer_doc_js";

/// HTML template variable name. The value contains Tp-Note's CSS code
/// to be included in the HTML rendition produced by the exporter.
pub const TMPL_HTML_VAR_EXPORTER_DOC_CSS: &str = "exporter_doc_css";

/// HTML template variable name. The value contains the highlighting CSS code
/// to be included in the HTML rendition produced by the exporter.
pub const TMPL_HTML_VAR_EXPORTER_HIGHLIGHTING_CSS: &str = "exporter_highlighting_css";

/// HTML template variable name. The value contains the path, for which the
/// viewer delivers Tp-Note's CSS code. Note, the viewer delivers the same CSS
/// code which is stored as value for `TMPL_HTML_VAR_VIEWER_DOC_CSS`.
pub const TMPL_HTML_VAR_VIEWER_DOC_CSS_PATH: &str = "viewer_doc_css_path";

/// The constant URL for which Tp-Note's internal web server delivers the CSS
/// style sheet. In HTML templates, this constant can be accessed as value of
/// the `TMPL_HTML_VAR_VIEWER_DOC_CSS_PATH` variable.
pub const TMPL_HTML_VAR_VIEWER_DOC_CSS_PATH_VALUE: &str = "/viewer_doc.css";

/// HTML template variable name. The value contains the path, for which the
/// viewer delivers Tp-Note's highlighting CSS code.
pub const TMPL_HTML_VAR_VIEWER_HIGHLIGHTING_CSS_PATH: &str = "viewer_highlighting_css_path";

/// The constant URL for which Tp-Note's internal web server delivers the CSS
/// style sheet. In HTML templates, this constant can be accessed as value of
/// the `TMPL_HTML_VAR_NOTE_CSS_PATH` variable.
pub const TMPL_HTML_VAR_VIEWER_HIGHLIGHTING_CSS_PATH_VALUE: &str = "/viewer_highlighting.css";

/// HTML template variable used in the error page containing the error message
/// explaining why this page could not be rendered.
#[allow(dead_code)]
pub const TMPL_HTML_VAR_DOC_ERROR: &str = "doc_error";

/// HTML template variable used in the error page containing a verbatim
/// HTML rendition with hyperlinks of the erroneous note file.
#[allow(dead_code)]
pub const TMPL_HTML_VAR_DOC_TEXT: &str = "doc_text";

lazy_static! {
/// Global variable containing the filename and template related configuration
/// data.
    pub static ref LIB_CFG: RwLock<LibCfg> = RwLock::new(LibCfg::default());
}

/// Configuration data, deserialized from the configuration file.
#[derive(Debug, Serialize, Deserialize)]
pub struct LibCfg {
    /// Configuration of filename parsing.
    pub filename: Filename,
    /// Configuration of content and filename templates.
    pub tmpl: Tmpl,
    /// Configuration of HTML templates.
    pub tmpl_html: TmplHtml,
}

/// Configuration of filename parsing, deserialized from the
/// configuration file.
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Filename {
    pub sort_tag_chars: String,
    pub sort_tag_separator: String,
    pub sort_tag_extra_separator: char,
    pub copy_counter_extra_separator: String,
    pub copy_counter_opening_brackets: String,
    pub copy_counter_closing_brackets: String,
    pub extension_default: String,
    pub extensions: Vec<(String, MarkupLanguage)>,
}

/// Filename templates and content templates, deserialized from the
/// configuration file.
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Tmpl {
    pub filter_assert_preconditions: Vec<(String, Vec<AssertPrecondition>)>,
    pub filter_get_lang: Vec<String>,
    pub filter_map_lang: Vec<Vec<String>>,
    pub filter_to_yaml_tab: u64,
    pub from_dir_content: String,
    pub from_dir_filename: String,
    pub from_clipboard_yaml_content: String,
    pub from_clipboard_yaml_filename: String,
    pub from_clipboard_content: String,
    pub from_clipboard_filename: String,
    pub from_text_file_content: String,
    pub from_text_file_filename: String,
    pub annotate_file_content: String,
    pub annotate_file_filename: String,
    pub sync_filename: String,
}

/// Configuration for the HTML exporter feature, deserialized from the
/// configuration file.
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct TmplHtml {
    pub viewer: String,
    pub viewer_error: String,
    pub viewer_doc_css: String,
    pub viewer_highlighting_theme: String,
    pub exporter: String,
    pub exporter_doc_css: String,
    pub exporter_highlighting_theme: String,
}

impl LibCfg {
    /// Perform some semantic consistency checks.
    /// * `sort_tag_extra_separator` must NOT be in `sort_tag_chars`.
    /// * `sort_tag_extra_separator` must NOT be in `0..9`.
    /// * `sort_tag_extra_separator` must NOT be in `a..z`.
    /// * `sort_tag_extra_separator` must NOT be in `sort_tag_chars`.
    /// * `sort_tag_extra_separator` must NOT `FILENAME_DOTFILE_MARKER`.
    /// * `copy_counter_extra_separator` must be one of
    ///   `sanitize_filename_reader_friendly::TRIM_LINE_CHARS`.
    /// * All characters of `sort_tag_separator` must be in `sort_tag_chars`.
    /// * `sort_tag_separator` must start with NOT `FILENAME_DOTFILE_MARKER`.
    pub fn assert_validity(&self) -> Result<(), LibCfgError> {
        // Check for obvious configuration errors.
        // * `sort_tag_extra_separator` must NOT be in `sort_tag_chars`.
        // * `sort_tag_extra_separator` must NOT `FILENAME_DOTFILE_MARKER`.
        if self
            .filename
            .sort_tag_chars
            .contains(self.filename.sort_tag_extra_separator)
            || (self.filename.sort_tag_extra_separator == FILENAME_DOTFILE_MARKER)
            || self.filename.sort_tag_extra_separator.is_ascii_digit()
            || self.filename.sort_tag_extra_separator.is_ascii_lowercase()
        {
            return Err(LibCfgError::SortTagExtraSeparator {
                dot_file_marker: FILENAME_DOTFILE_MARKER,
                chars: self.filename.sort_tag_chars.escape_default().to_string(),
                extra_separator: self
                    .filename
                    .sort_tag_extra_separator
                    .escape_default()
                    .to_string(),
            });
        }

        // Check for obvious configuration errors.
        // * All characters of `sort_tag_separator` must be in `sort_tag_chars`.
        // * `sort_tag_separator` must NOT start with `FILENAME_DOTFILE_MARKER`.
        // * `sort_tag_separator` must NOT contain ASCII `0..9` or `a..z`.
        if !self.filename.sort_tag_separator.chars().all(|c| {
            c.is_ascii_digit() || c.is_ascii_lowercase() || self.filename.sort_tag_chars.contains(c)
        }) || self
            .filename
            .sort_tag_separator
            .starts_with(FILENAME_DOTFILE_MARKER)
        {
            return Err(LibCfgError::SortTagSeparator {
                dot_file_marker: FILENAME_DOTFILE_MARKER,
                chars: self.filename.sort_tag_chars.escape_default().to_string(),
                separator: self
                    .filename
                    .sort_tag_separator
                    .escape_default()
                    .to_string(),
            });
        }

        // Check for obvious configuration errors.
        // * `copy_counter_extra_separator` must one of
        //   `sanitize_filename_reader_friendly::TRIM_LINE_CHARS`.
        if !TRIM_LINE_CHARS.contains(&self.filename.copy_counter_extra_separator) {
            return Err(LibCfgError::CopyCounterExtraSeparator {
                chars: TRIM_LINE_CHARS.escape_default().to_string(),
                extra_separator: self
                    .filename
                    .copy_counter_extra_separator
                    .escape_default()
                    .to_string(),
            });
        }

        // Assert that `filename.extension_default` is listed in
        // `filename.extensions[..].0`.
        if !self
            .filename
            .extensions
            .iter()
            .any(|ext| ext.0 == self.filename.extension_default)
        {
            return Err(LibCfgError::ExtensionDefault {
                extension_default: self.filename.extension_default.to_owned(),
                extensions: {
                    let mut list = self.filename.extensions.iter().fold(
                        String::new(),
                        |mut output, (k, _v)| {
                            let _ = write!(output, "{k}, ");
                            output
                        },
                    );
                    list.truncate(list.len().saturating_sub(2));
                    list
                },
            });
        }

        // Highlighting config is valid?
        // Validate `tmpl_html.viewer_highlighting_theme` and
        // `tmpl_html.exporter_highlighting_theme`.
        #[cfg(feature = "renderer")]
        {
            let ts = ThemeSet::load_defaults();
            let theme_name = &self.tmpl_html.viewer_highlighting_theme;
            if !theme_name.is_empty() && ts.themes.get(theme_name).is_none() {
                return Err(LibCfgError::ThemeName {
                    var: "viewer_highlighting_theme".to_string(),
                    value: theme_name.to_owned(),
                    available: ts.themes.into_keys().fold(String::new(), |mut output, k| {
                        let _ = write!(output, "{k}, ");
                        output
                    }),
                });
            };
            let theme_name = &self.tmpl_html.exporter_highlighting_theme;
            if !theme_name.is_empty() && ts.themes.get(theme_name).is_none() {
                return Err(LibCfgError::ThemeName {
                    var: "exporter_highlighting_theme".to_string(),
                    value: theme_name.to_owned(),
                    available: ts.themes.into_keys().fold(String::new(), |mut output, k| {
                        let _ = write!(output, "{k}, ");
                        output
                    }),
                });
            };
        }

        Ok(())
    }
}

/// Defaults are sourced from file `tpnote-lib/src/config_default.toml`.
impl Default for LibCfg {
    fn default() -> Self {
        toml::from_str(LIB_CONFIG_DEFAULT_TOML).expect(
            "Error in default configuration in source file:\n\
                 `tpnote-lib/src/config_default.toml`",
        )
    }
}

lazy_static! {
/// Global variable containing the filename and template related configuration
/// data.
    pub static ref LIB_CFG_CACHE: RwLock<LibCfgCache> = RwLock::new(LibCfgCache::new());
}

/// Configuration data, deserialized and preprocessed.
#[derive(Debug, Serialize, Deserialize)]
pub struct LibCfgCache {
    /// The result of an expensive calculation:
    /// `crate::highlight::get_viewer_highlighting_css()` with
    /// `lib_cfg.tmpl_html.viewer_highlighting_theme` as input.
    pub viewer_highlighting_css: String,
}

impl LibCfgCache {
    fn new() -> Self {
        Self {
            #[cfg(feature = "renderer")]
            viewer_highlighting_css: get_viewer_highlighting_css(),
            #[cfg(not(feature = "renderer"))]
            viewer_highlighting_css: String::new(),
        }
    }
}

/// Defines the way the HTML exporter rewrites local links.
///
/// The enum `LocalLinkKind` allows you to fine tune how local links are written
/// out. Valid variants are: `off`, `short` and `long`. In order to achieve
/// this, the user must respect the following convention concerning absolute
/// paths in local links in Tp-Note documents: When a document contains a
/// local link with an absolute path, the base of this path
/// is considered to be the the directory where the marker file `.tpnoteroot`
/// resides (or `/` in non exists). The option `--export-link- rewriting`
/// decides how local links in the Tp-Note  document are converted when the
/// HTML is generated.  If its value is `short`, then local links with relative
/// paths are converted to absolute paths. The base of the resulting path is
/// where the `.tpnoteroot` file resides (or `/` if none exists).
/// Consider the following example `LocalLinkKind::Short`:
///
/// * The Tp-Note file `/my/docs/car/bill.md` contains
/// * a local link with an absolute path: `/car/scan.jpg`,
/// * and another link with a relative path: `./photo.jpg`.
/// * The document root marker is: `/my/docs/.tpnoteroot`.
///
/// The images in the resulting HTML will appear as
///
/// * `/car/scan.jpg`.
/// * `/car/photo.jpg`.
///
/// For `LocalLinkKind::Long`, in addition to the above, all absolute paths in
/// local links are rebased to `/`'. Consider the following example:
///
/// * The Tp-Note file `/my/docs/car/bill.md` contains
/// * a link with an absolute path: `/car/scan.jpg`,
/// * and another link with a relative path: `./photo.jpg`.
/// * The document root marker is: `/my/docs/.tpnoteroot`.
///
/// The images in the resulting HTML will appear as
///
/// * `/my/docs/car/scan.jpg`.
/// * `/my/docs/car/photo.jpg`.
///
#[derive(Debug, Hash, Clone, Eq, PartialEq, Deserialize, Serialize, Copy, Default)]
pub enum LocalLinkKind {
    /// Do not rewrite links.
    Off,
    /// Rewrite relative local links. Base: ".tpnoteroot"
    Short,
    /// Rewrite all local links. Base: "/"
    #[default]
    Long,
}

impl FromStr for LocalLinkKind {
    type Err = LibCfgError;
    fn from_str(level: &str) -> Result<LocalLinkKind, Self::Err> {
        match &*level.to_ascii_lowercase() {
            "off" => Ok(LocalLinkKind::Off),
            "short" => Ok(LocalLinkKind::Short),
            "long" => Ok(LocalLinkKind::Long),
            _ => Err(LibCfgError::ParseLocalLinkKind {}),
        }
    }
}

/// Describes a set of tests, that assert template variable `tera:Value`
/// properties.
#[derive(Default, Debug, Hash, Clone, Eq, PartialEq, Deserialize, Serialize, Copy)]
pub enum AssertPrecondition {
    /// `IsDefined`: Assert that the variable is defined in the template.
    IsDefined,
    /// `IsNotEmptyString`: In addition to `IsString`, the condition asserts,
    ///  that the string -or all substrings-) are not empty.
    IsNotEmptyString,
    /// `IsString`: Assert, that if the variable is defined, its type -or all
    ///  subtypes- are `Value::String`.
    IsString,
    /// `IsNumber`: Assert, that if the variable is defined, its type -or all
    ///  subtypes- are `Value::Number`.
    IsNumber,
    /// `IsBool`: Assert, that if the variable is defined, its type -or all
    ///  subtypes- are `Value::Bool`.
    IsBool,
    /// `IsNotCompound`: Assert, that if the variable is defined, its type is
    /// not `Value::Array` or `Value::Object`.
    IsNotCompound,
    /// `IsValidSortTag`: Assert, that if the variable is defined, the value's
    ///  string representation contains solely characters of the
    ///  `filename.sort_tag_chars` set, digits or lowercase letters.
    ///  The number of lowercase letters in a row is limited by
    ///  `tpnote_lib::config::FILENAME_SORT_TAG_LETTERS_IN_SUCCESSION_MAX`.
    IsValidSortTag,
    /// `IsTpnoteExtension`: Assert, that if the variable is defined,
    ///  the values string representation is registered in one of the
    ///  `filename.extension_*` configuration file variables.
    IsTpnoteExtension,
    /// `NoOperation` (default): A test that is always satisfied. For internal
    ///  use only.
    #[default]
    NoOperation,
}
