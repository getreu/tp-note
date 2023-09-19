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
use crate::error::LibCfgError;
#[cfg(feature = "renderer")]
use crate::highlight::get_css;
use lazy_static::lazy_static;
use parking_lot::RwLock;
use sanitize_filename_reader_friendly::TRIM_LINE_CHARS;
use serde::{Deserialize, Serialize};
use std::str::FromStr;

/// Default configuragtion.
pub(crate) const LIB_CFG_TOML: &str = include_str!("config_default.toml");

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

/// The apperance of a file with this filename marks the position of
/// `TMPL_VAR_ROOT_PATH`.
pub const FILENAME_ROOT_PATH_MARKER: &str = ".tpnote.toml";

/// When a filename is taken already, Tp-Note adds a copy
/// counter number in the range of `0..COPY_COUNTER_MAX`
/// at the end.
pub const FILENAME_COPY_COUNTER_MAX: usize = 400;

/// This a dot by definition.
pub const FILENAME_DOTFILE_MARKER: char = '.';

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
/// Under Windows, the user's language tag is queried through the WinAPI.
/// If defined, the environment variable `TPNOTE_LANG` overwrites this value
/// (all operating systems).
pub const TMPL_VAR_LANG: &str = "lang";
/// All the front matter fields serialized as text, exactly as they appear in
/// the front matter.
pub const TMPL_VAR_NOTE_FM_TEXT: &str = "note_fm_text";

/// Contains the body of the file the command line option `<path>`
/// points to. Only available in the `TMPL_FROM_TEXT_FILE_CONTENT`,
/// `TMPL_SYNC_FILENAME` and HTML templates.
pub const TMPL_VAR_NOTE_BODY_TEXT: &str = "note_body_text";

/// Contains the date of the file the command line option `<path>` points to.
/// The date is represented as an integer the way `std::time::SystemTime`
/// resolves to on the platform. Only available in the
/// `TMPL_FROM_TEXT_FILE_CONTENT`, `TMPL_SYNC_FILENAME` and HTML templates.
pub const TMPL_VAR_NOTE_FILE_DATE: &str = "note_file_date";

/// Prefix prepended to front matter field names when a template variable
/// is generated with the same name.
pub const TMPL_VAR_FM_: &str = "fm_";

/// Contains a Hash Map with all front matter fields. Lists are flattened
/// into strings. These variables are only available in the
/// `TMPL_FROM_TEXT_FILE_CONTENT`, `TMPL_SYNC_FILENAME` and HTML templates.
pub const TMPL_VAR_FM_ALL: &str = "fm_all";

/// By default, the template `TMPL_SYNC_FILENAME` defines the function of
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

/// By default, the template `TMPL_SYNC_FILENAME` defines the function of
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

/// HTML template variable containing the note's body.
/// We could set
/// `#[cfg(feature = "viewer")]`,
/// but we prefer the same config file structure independent
/// of the enabled features.
pub const TMPL_HTML_VAR_NOTE_BODY_HTML: &str = "note_body_html";

/// HTML template variable containing the automatically generated JavaScript
/// code to be included in the HTML rendition.
/// We could set
/// `#[cfg(feature = "viewer")]`,
/// but we prefer the same config file structure independent
/// of the enabled features.
pub const TMPL_HTML_VAR_NOTE_JS: &str = "note_js";

/// HTML template variable name. The value contains the highlighting CSS code
/// to be included in the HTML rendition produced by the exporter.
pub const TMPL_HTML_VAR_NOTE_CSS: &str = "note_css";

/// HTML template variable name. The value contains the path, for which
/// the viewer delivers CSS code. Note, the viewer delivers the same CSS code
/// which is stored as value for `TMPL_VAR_NOTE_CSS`.
pub const TMPL_HTML_VAR_NOTE_CSS_PATH: &str = "note_css_path";

/// The constant URL for which Tp-Note's internal web server delivers the CSS
/// stylesheet. In HTML templates, this constant can be accessed as value of
/// the `TMPL_VAR_NOTE_CSS_PATH` variable.
pub const TMPL_HTML_VAR_NOTE_CSS_PATH_VALUE: &str = "/tpnote.css";

/// HTML template variable used in the error page containing the error message
/// explaining why this page could not be rendered.
/// We could set
/// `#[cfg(feature = "viewer")]`,
/// but we prefer the same config file structure independent
/// of the enabled features.
#[allow(dead_code)]
pub const TMPL_HTML_VAR_NOTE_ERROR: &str = "note_error";

/// HTML template variable used in the error page containing a verbatim
/// HTML rendition with hyperlinks of the erroneous note file.
/// We could set
/// `#[cfg(feature = "viewer")]`,
/// but we prefer the same config file structure independent
/// of the enabled features.
#[allow(dead_code)]
pub const TMPL_HTML_VAR_NOTE_ERRONEOUS_CONTENT_HTML: &str = "note_erroneous_content_html";

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

/// Deserialise the default configuration.
impl ::std::default::Default for LibCfg {
    fn default() -> Self {
        let mut config: LibCfg = toml::from_str(LIB_CFG_TOML)
            .expect("can not parse included configuration file `tpnote_lib.toml`");

        #[allow(unused_mut)]
        let mut css = config.tmpl_html.css.to_owned();
        #[cfg(feature = "renderer")]
        css.push_str(&get_css());
        config.tmpl_html.css = css;
        config
    }
}

/// Configuration of filename parsing, deserialized from the
/// configuration file.
#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct Filename {
    pub sort_tag_chars: String,
    pub sort_tag_separator: String,
    pub sort_tag_extra_separator: char,
    pub copy_counter_extra_separator: String,
    pub copy_counter_opening_brackets: String,
    pub copy_counter_closing_brackets: String,
    pub extension_default: String,
    pub extensions_md: Vec<String>,
    pub extensions_rst: Vec<String>,
    pub extensions_html: Vec<String>,
    pub extensions_txt: Vec<String>,
    pub extensions_no_viewer: Vec<String>,
}

/// Filename templates and content templates, deserialized from the
/// configuration file.
#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct Tmpl {
    pub filter_assert_preconditions: Vec<(String, Vec<AssertPrecondition>)>,
    pub filter_get_lang: Vec<String>,
    pub filter_map_lang: Vec<Vec<String>>,
    pub filter_to_yaml_tab: u64,
    pub new_content: String,
    pub new_filename: String,
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
#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct TmplHtml {
    pub viewer: String,
    pub viewer_error: String,
    pub exporter: String,
    /// Configuration variable holding the source code highlighter CSS code
    /// concatenated with `TMPL_HTML_CSS_COMMON`. In HTML templates this
    /// constant can be accessed as value of the `TMPL_HTML_VAR_NOTE_CSS`
    /// variable.
    pub css: String,
}

impl LibCfg {
    /// Perfom some sematic consitency checks.
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
            .find(self.filename.sort_tag_extra_separator)
            .is_some()
            || self.filename.sort_tag_extra_separator == FILENAME_DOTFILE_MARKER
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
        if !self
            .filename
            .sort_tag_separator
            .chars()
            .all(|c| self.filename.sort_tag_chars.contains(c))
            || self
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

        Ok(())
    }
}

/// Defines the way the HTML exporter rewrites local links.
///
/// The enum `LocalLinkKind` allows you to fine tune how local links are written
/// out. Valid variants are: `off`, `short` and `long`. In order to achieve
/// this, the user must respect  the following convention concerning absolute
/// local links in Tp-Note documents:  The base of absolute local links in Tp-
/// Note documents must be the directory where the marker file `.tpnoteroot`
/// resides (or `/` in non exists). The option `--export-link- rewriting`
/// decides how local links in the Tp-Note  document are converted when the
/// HTML is generated.  If its value is `short`, then relative local links are
/// converted to absolute links. The base of the resulting links is where the
/// `.tpnoteroot` file resides (or `/` if none exists). Consider the following
/// example:
///
/// * The Tp-Note file `/my/docs/car/bill.md` contains
/// * the absolute link `/car/scan.jpg`.
/// * and the relative link `./photo.jpg`.
/// * The document root marker is: `/my/docs/.tpnoteroot`.
///
/// The images in the resulting HTML will appear as
///
/// * `/car/scan.jpg`.
/// * `/car/photo.jpg`.
///
/// For `LocalLinkKind::long`, in addition to the above, all absolute
/// local links are rebased to `/`'. Consider the following example:
///
/// * The Tp-Note file `/my/docs/car/bill.md` contains
/// * the absolute link `/car/scan.jpg`.
/// * and the relative link `./photo.jpg`.
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
    /// Assert that the variable is defined in the template
    IsDefined,
    /// In addtion to `IsString`, the condition asserts, that the string is
    /// not empty.
    IsNotEmptyString,
    /// Assert, that if the variable is defined, its type is `Value::String`.
    IsString,
    /// Assert, that if the variable is defined, its type is `Value::Number`.
    IsNumber,
    /// Assert, that if the variable is defined, its type is `Value::Bool`.
    IsBool,
    /// Assert, that if the variable is defined, its type is not `Value::Array`
    /// or `Value::Object`.
    IsNotCompound,
    /// Assert, that if the variable is defined, the values
    /// string representation contains solely characters of the
    /// `filename.sort_tag_chars` set.
    HasOnlySortTagChars,
    /// Assert, that if the variable is defined, the values string
    /// representation is regeistered in one of the `filename.extension_*`
    /// configuraion file variables.
    IsTpnoteExtension,
    /// A test that is always satisfied. For internal use only.
    #[default]
    NoOperation,
}

impl FromStr for AssertPrecondition {
    type Err = LibCfgError;
    fn from_str(precondition: &str) -> Result<AssertPrecondition, Self::Err> {
        match precondition {
            "IsDefined" => Ok(AssertPrecondition::IsDefined),
            "IsNotEmptyString" => Ok(AssertPrecondition::IsNotEmptyString),
            "IsString" => Ok(AssertPrecondition::IsString),
            "IsNumber" => Ok(AssertPrecondition::IsNumber),
            "IsBool" => Ok(AssertPrecondition::IsBool),
            "IsNotCompound" => Ok(AssertPrecondition::IsNotCompound),
            "HasOnlySortTagChars" => Ok(AssertPrecondition::HasOnlySortTagChars),
            _ => Err(LibCfgError::ParseAssertPrecondition {}),
        }
    }
}
