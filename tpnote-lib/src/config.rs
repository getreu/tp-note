//! Set configuration defaults by reading the internal default
//! configuration file `LIB_CONFIG_DEFAULT_TOML`. After processing, the
//! configuration data is exposed via the variable `LIB_CFG` behind a
//! mutex. This makes it possible to modify all configuration defaults
//! (including templates) at runtime.
//!
//! ```rust
//! use tpnote_lib::config::LIB_CFG;
//!
//! let mut lib_cfg = LIB_CFG.write();
//! let i = lib_cfg.scheme_idx("default").unwrap();
//! (*lib_cfg).scheme[i].filename.copy_counter.extra_separator = '@'.to_string();
//! ```
//!
//! Contract to be uphold by the user of this API:
//! seeing that `LIB_CFG` is mutable at runtime, it must be sourced before the
//! start of Tp-Note. All modification of `LIB_CFG` is terminated before
//! accessing the high-level API in the `workflow` module of this crate.

use crate::config_value::CfgVal;
use crate::error::LibCfgError;
#[cfg(feature = "lang-detection")]
use crate::filter::IsoCode639_1;
#[cfg(feature = "renderer")]
use crate::highlight::get_highlighting_css;
use crate::markup_language::InputConverter;
use crate::markup_language::MarkupLanguage;
use parking_lot::RwLock;
use sanitize_filename_reader_friendly::TRIM_LINE_CHARS;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt::Write;
use std::str::FromStr;
use std::sync::LazyLock;
#[cfg(feature = "renderer")]
use syntect::highlighting::ThemeSet;
use toml::Value;

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
pub const FILENAME_ROOT_PATH_MARKER: &str = "tpnote.toml";

/// When a filename is taken already, Tp-Note adds a copy
/// counter number in the range of `0..COPY_COUNTER_MAX`
/// at the end.
pub const FILENAME_COPY_COUNTER_MAX: usize = 400;

/// A filename extension, if present, is separated by a dot.
pub(crate) const FILENAME_EXTENSION_SEPARATOR_DOT: char = '.';

/// A dotfile starts with a dot.
pub(crate) const FILENAME_DOTFILE_MARKER: char = '.';

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
/// that upwards from `TMPL_VAR_DIR_PATH` contains a file named
/// `FILENAME_ROOT_PATH_MARKER`. The root directory is used by Tp-Note's viewer
/// as base directory
pub const TMPL_VAR_ROOT_PATH: &str = "root_path";

/// Contains the YAML header (if any) of the HTML clipboard content.
/// Otherwise the empty string.
/// Note: as current HTML clipboard provider never send YAML headers (yet),
/// expect this to be empty.
pub const TMPL_VAR_HTML_CLIPBOARD_HEADER: &str = "html_clipboard_header";

/// If there is a meta header in the HTML clipboard, this contains
/// the body only. Otherwise, it contains the whole clipboard content.
/// Note: as current HTML clipboard provider never send YAML headers (yet),
/// expect this to be the whole HTML clipboard.
pub const TMPL_VAR_HTML_CLIPBOARD: &str = "html_clipboard";

/// Contains the YAML header (if any) of the plain text clipboard content.
/// Otherwise the empty string.
pub const TMPL_VAR_TXT_CLIPBOARD_HEADER: &str = "txt_clipboard_header";

/// If there is a YAML header in the plain text clipboard, this contains
/// the body only. Otherwise, it contains the whole clipboard content.
pub const TMPL_VAR_TXT_CLIPBOARD: &str = "txt_clipboard";

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
/// Note: this variable might not be defined with some file systems or on some
/// platforms.  
pub const TMPL_VAR_DOC_FILE_DATE: &str = "doc_file_date";

/// Prefix prepended to front matter field names when a template variable
/// is generated with the same name.
pub const TMPL_VAR_FM_: &str = "fm_";

/// Contains a Hash Map with all front matter fields. Lists are flattened
/// into strings. These variables are only available in the
/// `tmpl.from_text_file_content`, `tmpl.sync_filename` and HTML templates.
pub const TMPL_VAR_FM_ALL: &str = "fm";

/// If present, this header variable can switch the `settings.current_theme`
/// before the filename template is processed.
pub const TMPL_VAR_FM_SCHEME: &str = "fm_scheme";

/// By default, the template `tmpl.sync_filename` defines the function of this
/// variable as follows:
/// Contains the value of the front matter field `file_ext` and determines the
/// markup language used to render the document. When the field is missing the
/// markup language is derived from the note's filename extension.
///
/// This is a dynamically generated variable originating from the front matter
/// of the current note. As all front matter variables, its value is copied as
/// it is without modification. Here, the only special treatment is, when
/// analyzing the front matter, it is verified, that the value of this variable
/// is registered in one of the `filename.extensions_*` variables.
pub const TMPL_VAR_FM_FILE_EXT: &str = "fm_file_ext";

/// By default, the template `tmpl.sync_filename` defines the function of this
/// variable as follows:
/// If this variable is defined, the _sort tag_ of the filename is replaced with
/// the value of this variable next time the filename is synchronized. If not
/// defined, the sort tag of the filename is never changed.
///
/// This is a dynamically generated variable originating from the front matter
/// of the current note. As all front matter variables, its value is copied as
/// it is without modification. Here, the only special treatment is, when
/// analyzing the front matter, it is verified, that all the characters of the
/// value of this variable are listed in `filename.sort_tag.extra_chars`.
pub const TMPL_VAR_FM_SORT_TAG: &str = "fm_sort_tag";

/// Contains the value of the front matter field `no_filename_sync`. When set
/// to `no_filename_sync:` or `no_filename_sync: true`, the filename
/// synchronization mechanism is disabled for this note file. Depreciated
/// in favor of `TMPL_VAR_FM_FILENAME_SYNC`.
pub const TMPL_VAR_FM_NO_FILENAME_SYNC: &str = "fm_no_filename_sync";

/// Contains the value of the front matter field `filename_sync`. When set to
/// `filename_sync: false`, the filename synchronization mechanism is
/// disabled for this note file. Default value is `true`.
pub const TMPL_VAR_FM_FILENAME_SYNC: &str = "fm_filename_sync";

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

/// Global variable containing the filename and template related configuration
/// data. This can be changed by the consumer of this library. Once the
/// initialization done, this should remain static.
/// For session configuration see: `settings::SETTINGS`.
pub static LIB_CFG: LazyLock<RwLock<LibCfg>> = LazyLock::new(|| RwLock::new(LibCfg::default()));

/// An array of field names after deserialization.
pub const LIB_CFG_RAW_FIELD_NAMES: [&str; 4] =
    ["scheme_sync_default", "base_scheme", "scheme", "tmpl_html"];

/// Processed configuration data.
///
/// Its structure is different form the input form defined in `LibCfgRaw` (see
/// example in `LIB_CONFIG_DEFAULT_TOML`).
/// For conversion use:
///
/// ```rust
/// use tpnote_lib::config::LIB_CONFIG_DEFAULT_TOML;
/// use tpnote_lib::config::LibCfg;
/// use tpnote_lib::config_value::CfgVal;
/// use std::str::FromStr;
///
/// let cfg_val = CfgVal::from_str(LIB_CONFIG_DEFAULT_TOML).unwrap();
///
/// // Run test.
/// let lib_cfg = LibCfg::try_from(cfg_val).unwrap();
///
/// // Check.
/// assert_eq!(lib_cfg.scheme_sync_default, "default")
/// ```
#[derive(Debug, Serialize, Deserialize)]
#[serde(try_from = "LibCfgIntermediate")]
pub struct LibCfg {
    /// The fallback scheme for the `sync_filename` template choice, if the
    /// `scheme` header variable is empty or is not defined.
    pub scheme_sync_default: String,
    /// Configuration of `Scheme`.
    pub scheme: Vec<Scheme>,
    /// Configuration of HTML templates.
    pub tmpl_html: TmplHtml,
}

/// Unprocessed configuration data, deserialized from the configuration file.
/// This is an intermediate representation of `LibCfg`.
/// This defines the structure of the configuration file.
/// Its default values are stored in serialized form in
/// `LIB_CONFIG_DEFAULT_TOML`.
#[derive(Debug, Serialize, Deserialize)]
struct LibCfgIntermediate {
    /// The fallback scheme for the `sync_filename` template choice, if the
    /// `scheme` header variable is empty or is not defined.
    pub scheme_sync_default: String,
    /// This is the base scheme, from which all instantiated schemes inherit.
    pub base_scheme: Value,
    /// This flatten into a `scheme=Vec<Scheme>` in which the `Scheme`
    /// definitions are not complete. Only after merging it into a copy of
    /// `base_scheme` we can parse it into a `Scheme` structs. The result is not
    /// kept here, it is stored into `LibCfg` struct instead.
    #[serde(flatten)]
    pub scheme: HashMap<String, Value>,
    /// Configuration of HTML templates.
    pub tmpl_html: TmplHtml,
}

impl LibCfg {
    /// Returns the index of a named scheme. If no scheme with that name can be
    /// found, return `LibCfgError::SchemeNotFound`.
    pub fn scheme_idx(&self, name: &str) -> Result<usize, LibCfgError> {
        self.scheme
            .iter()
            .enumerate()
            .find(|&(_, scheme)| scheme.name == name)
            .map_or_else(
                || {
                    Err(LibCfgError::SchemeNotFound {
                        scheme_name: name.to_string(),
                        schemes: {
                            //Already imported: `use std::fmt::Write;`
                            let mut errstr =
                                self.scheme.iter().fold(String::new(), |mut output, s| {
                                    let _ = write!(output, "{}, ", s.name);
                                    output
                                });
                            errstr.truncate(errstr.len().saturating_sub(2));
                            errstr
                        },
                    })
                },
                |(i, _)| Ok(i),
            )
    }
    /// Perform some semantic consistency checks.
    /// * `sort_tag.extra_separator` must NOT be in `sort_tag.extra_chars`.
    /// * `sort_tag.extra_separator` must NOT be in `0..9`.
    /// * `sort_tag.extra_separator` must NOT be in `a..z`.
    /// * `sort_tag.extra_separator` must NOT be in `sort_tag.extra_chars`.
    /// * `sort_tag.extra_separator` must NOT `FILENAME_DOTFILE_MARKER`.
    /// * `copy_counter.extra_separator` must be one of
    ///   `sanitize_filename_reader_friendly::TRIM_LINE_CHARS`.
    /// * All characters of `sort_tag.separator` must be in `sort_tag.extra_chars`.
    /// * `sort_tag.separator` must start with NOT `FILENAME_DOTFILE_MARKER`.
    pub fn assert_validity(&self) -> Result<(), LibCfgError> {
        for scheme in &self.scheme {
            // Check for obvious configuration errors.
            // * `sort_tag.extra_separator` must NOT be in `sort_tag.extra_chars`.
            // * `sort_tag.extra_separator` must NOT `FILENAME_DOTFILE_MARKER`.
            if scheme
                .filename
                .sort_tag
                .extra_chars
                .contains(scheme.filename.sort_tag.extra_separator)
                || (scheme.filename.sort_tag.extra_separator == FILENAME_DOTFILE_MARKER)
                || scheme.filename.sort_tag.extra_separator.is_ascii_digit()
                || scheme
                    .filename
                    .sort_tag
                    .extra_separator
                    .is_ascii_lowercase()
            {
                return Err(LibCfgError::SortTagExtraSeparator {
                    scheme_name: scheme.name.to_string(),
                    dot_file_marker: FILENAME_DOTFILE_MARKER,
                    sort_tag_extra_chars: scheme
                        .filename
                        .sort_tag
                        .extra_chars
                        .escape_default()
                        .to_string(),
                    extra_separator: scheme
                        .filename
                        .sort_tag
                        .extra_separator
                        .escape_default()
                        .to_string(),
                });
            }

            // Check for obvious configuration errors.
            // * All characters of `sort_tag.separator` must be in `sort_tag.extra_chars`.
            // * `sort_tag.separator` must NOT start with `FILENAME_DOTFILE_MARKER`.
            // * `sort_tag.separator` must NOT contain ASCII `0..9` or `a..z`.
            if !scheme.filename.sort_tag.separator.chars().all(|c| {
                c.is_ascii_digit()
                    || c.is_ascii_lowercase()
                    || scheme.filename.sort_tag.extra_chars.contains(c)
            }) || scheme
                .filename
                .sort_tag
                .separator
                .starts_with(FILENAME_DOTFILE_MARKER)
            {
                return Err(LibCfgError::SortTagSeparator {
                    scheme_name: scheme.name.to_string(),
                    dot_file_marker: FILENAME_DOTFILE_MARKER,
                    chars: scheme
                        .filename
                        .sort_tag
                        .extra_chars
                        .escape_default()
                        .to_string(),
                    separator: scheme
                        .filename
                        .sort_tag
                        .separator
                        .escape_default()
                        .to_string(),
                });
            }

            // Check for obvious configuration errors.
            // * `copy_counter.extra_separator` must one of
            //   `sanitize_filename_reader_friendly::TRIM_LINE_CHARS`.
            if !TRIM_LINE_CHARS.contains(&scheme.filename.copy_counter.extra_separator) {
                return Err(LibCfgError::CopyCounterExtraSeparator {
                    scheme_name: scheme.name.to_string(),
                    chars: TRIM_LINE_CHARS.escape_default().to_string(),
                    extra_separator: scheme
                        .filename
                        .copy_counter
                        .extra_separator
                        .escape_default()
                        .to_string(),
                });
            }

            // Assert that `filename.extension_default` is listed in
            // `filename.extensions[..].0`.
            if !scheme
                .filename
                .extensions
                .iter()
                .any(|ext| ext.0 == scheme.filename.extension_default)
            {
                return Err(LibCfgError::ExtensionDefault {
                    scheme_name: scheme.name.to_string(),
                    extension_default: scheme.filename.extension_default.to_owned(),
                    extensions: {
                        let mut list = scheme.filename.extensions.iter().fold(
                            String::new(),
                            |mut output, (k, _v1, _v2)| {
                                let _ = write!(output, "{k}, ");
                                output
                            },
                        );
                        list.truncate(list.len().saturating_sub(2));
                        list
                    },
                });
            }

            if let Mode::Error(e) = &scheme.tmpl.filter.get_lang.mode {
                return Err(e.clone());
            }

            // Assert that `filter.get_lang.minimum_relative_distance` is
            // between `0.0` and `0.99`.
            let dist = scheme.tmpl.filter.get_lang.minimum_relative_distance;
            if !(0.0..=0.99).contains(&dist) {
                return Err(LibCfgError::MinimumRelativeDistanceInvalid {
                    scheme_name: scheme.name.to_string(),
                    dist,
                });
            }
        }

        // Highlighting config is valid?
        // Validate `tmpl_html.viewer_highlighting_theme` and
        // `tmpl_html.exporter_highlighting_theme`.
        #[cfg(feature = "renderer")]
        {
            let hl_theme_set = ThemeSet::load_defaults();
            let hl_theme_name = &self.tmpl_html.viewer_highlighting_theme;
            if !hl_theme_name.is_empty() && !hl_theme_set.themes.contains_key(hl_theme_name) {
                return Err(LibCfgError::HighlightingThemeName {
                    var: "viewer_highlighting_theme".to_string(),
                    value: hl_theme_name.to_owned(),
                    available: hl_theme_set.themes.into_keys().fold(
                        String::new(),
                        |mut output, k| {
                            let _ = write!(output, "{k}, ");
                            output
                        },
                    ),
                });
            };
            let hl_theme_name = &self.tmpl_html.exporter_highlighting_theme;
            if !hl_theme_name.is_empty() && !hl_theme_set.themes.contains_key(hl_theme_name) {
                return Err(LibCfgError::HighlightingThemeName {
                    var: "exporter_highlighting_theme".to_string(),
                    value: hl_theme_name.to_owned(),
                    available: hl_theme_set.themes.into_keys().fold(
                        String::new(),
                        |mut output, k| {
                            let _ = write!(output, "{k}, ");
                            output
                        },
                    ),
                });
            };
        }

        Ok(())
    }
}

/// Reads the file `./config_default.toml` (`LIB_CONFIG_DEFAULT_TOML`) into
/// `LibCfg`. Panics if this is not possible.
impl Default for LibCfg {
    fn default() -> Self {
        toml::from_str(LIB_CONFIG_DEFAULT_TOML)
            .expect("Error parsing LIB_CONFIG_DEFAULT_TOML into LibCfg")
    }
}

impl TryFrom<LibCfgIntermediate> for LibCfg {
    type Error = LibCfgError;

    /// Constructor expecting a `LibCfgRaw` struct as input.
    /// The variables `LibCfgRaw.scheme`,
    /// `LibCfgRaw.html_tmpl.viewer_highlighting_css` and
    /// `LibCfgRaw.html_tmpl.exporter_highlighting_css` are processed before
    /// storing in `Self`:
    /// 1. The entries in `LibCfgRaw.scheme` are merged into copies of
    ///    `LibCfgRaw.base_scheme` and the results are stored in `LibCfg.scheme`
    /// 2. If `LibCfgRaw.html_tmpl.viewer_highlighting_css` is empty,
    ///    a css is calculated from `tmpl.viewer_highlighting_theme`
    ///    and stored in `LibCfg.html_tmpl.viewer_highlighting_css`.
    /// 3.  Do the same for `LibCfgRaw.html_tmpl.exporter_highlighting_css`.
    fn try_from(lib_cfg_raw: LibCfgIntermediate) -> Result<Self, Self::Error> {
        let mut raw = lib_cfg_raw;
        // Now we merge all `scheme` into a copy of `base_scheme` and
        // parse the result into a `Vec<Scheme>`.
        //
        // Here we keep the result after merging and parsing.
        let mut schemes: Vec<Scheme> = vec![];
        // Get `theme`s in `config` as toml array. Clears the map as it is not
        // needed any more.
        if let Some(toml::Value::Array(lib_cfg_scheme)) = raw
            .scheme
            .drain()
            // Silently ignore all potential toml variables other than `scheme`.
            .filter(|(k, _)| k == "scheme")
            .map(|(_, v)| v)
            .next()
        {
            // Merge all `s` into a `base_scheme`, parse the result into a `Scheme`
            // and collect a `Vector`. `merge_depth=0` means we never append
            // to left hand arrays, we always overwrite them.
            schemes = lib_cfg_scheme
                .into_iter()
                .map(|v| CfgVal::merge_toml_values(raw.base_scheme.clone(), v, 0))
                .map(|v| v.try_into().map_err(|e| e.into()))
                .collect::<Result<Vec<Scheme>, LibCfgError>>()?;
        }
        let raw = raw; // Freeze.

        let mut tmpl_html = raw.tmpl_html;
        // Now calculate `LibCfgRaw.tmpl_html.viewer_highlighting_css`:
        #[cfg(feature = "renderer")]
        let css = if !tmpl_html.viewer_highlighting_css.is_empty() {
            tmpl_html.viewer_highlighting_css
        } else {
            get_highlighting_css(&tmpl_html.viewer_highlighting_theme)
        };
        #[cfg(not(feature = "renderer"))]
        let css = String::new();

        tmpl_html.viewer_highlighting_css = css;

        // Calculate `LibCfgRaw.tmpl_html.exporter_highlighting_css`:
        #[cfg(feature = "renderer")]
        let css = if !tmpl_html.exporter_highlighting_css.is_empty() {
            tmpl_html.exporter_highlighting_css
        } else {
            get_highlighting_css(&tmpl_html.exporter_highlighting_theme)
        };
        #[cfg(not(feature = "renderer"))]
        let css = String::new();

        tmpl_html.exporter_highlighting_css = css;

        // Store the result:
        let res = LibCfg {
            // Copy the parts of `config` into `LIB_CFG`.
            scheme_sync_default: raw.scheme_sync_default,
            scheme: schemes,
            tmpl_html,
        };
        // Perform some additional semantic checks.
        res.assert_validity()?;
        Ok(res)
    }
}

/// This constructor accepts as input the newtype `CfgVal` containing
/// a `toml::map::Map<String, Value>`. Each `String` is the name of a top
/// level configuration variable.
/// The inner Map is expected to be a data structure that can be copied into
/// the internal temporary variable `LibCfgRaw`. This internal variable
/// is then processed and the result is stored in a `LibCfg` struct. For details
/// see the `impl TryFrom<LibCfgRaw> for LibCfg`. The processing occurs as
/// follows:
///
/// 1. Merge each incomplete `CfgVal(key="scheme")` into
///    `CfgVal(key="base_scheme")` and
///    store the resulting `scheme` struct in `LibCfg.scheme`.
/// 2. If `CfgVal(key="html_tmpl.viewer_highlighting_css")` is empty, generate
///    the value from `CfgVal(key="tmpl.viewer_highlighting_theme")`.
/// 3. Do the same for `CfgVal(key="html_tmpl.exporter_highlighting_css")`.
impl TryFrom<CfgVal> for LibCfg {
    type Error = LibCfgError;

    fn try_from(cfg_val: CfgVal) -> Result<Self, Self::Error> {
        let value: toml::Value = cfg_val.into();
        Ok(value.try_into()?)
    }
}

/// Configuration data, deserialized from the configuration file.
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Scheme {
    pub name: String,
    /// Configuration of filename parsing.
    pub filename: Filename,
    /// Configuration of content and filename templates.
    pub tmpl: Tmpl,
}

/// Configuration of filename parsing, deserialized from the
/// configuration file.
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Filename {
    pub sort_tag: SortTag,
    pub copy_counter: CopyCounter,
    pub extension_default: String,
    pub extensions: Vec<(String, InputConverter, MarkupLanguage)>,
}

/// Configuration for sort-tag.
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct SortTag {
    pub extra_chars: String,
    pub separator: String,
    pub extra_separator: char,
    pub letters_in_succession_max: u8,
    pub sequential: Sequential,
}

/// Requirements for chronological sort tags.
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Sequential {
    pub digits_in_succession_max: u8,
}

/// Configuration for copy-counter.
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct CopyCounter {
    pub extra_separator: String,
    pub opening_brackets: String,
    pub closing_brackets: String,
}

/// Filename templates and content templates, deserialized from the
/// configuration file.
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Tmpl {
    pub fm_var: FmVar,
    pub filter: Filter,
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

/// Configuration describing how to localize and check front matter variables.
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct FmVar {
    pub localization: Vec<(String, String)>,
    pub assertions: Vec<(String, Vec<Assertion>)>,
}

/// Configuration related to various Tera template filters.
#[derive(Default, Debug, Clone, PartialEq, Deserialize, Serialize)]
pub struct Filter {
    pub get_lang: GetLang,
    pub map_lang: Vec<Vec<String>>,
    pub to_yaml_tab: u64,
}

/// Configuration related to various Tera template filters.
#[derive(Default, Debug, Clone, PartialEq, Deserialize, Serialize)]
#[serde(try_from = "GetLangIntermediate")]
pub struct GetLang {
    pub mode: Mode,
    #[cfg(feature = "lang-detection")]
    pub language_candidates: Vec<IsoCode639_1>,
    #[cfg(not(feature = "lang-detection"))]
    pub language_candidates: Vec<String>,
    pub minimum_relative_distance: f64,
}

/// Configuration related to various Tera template filters.
#[derive(Default, Debug, Clone, PartialEq, Deserialize, Serialize)]
struct GetLangIntermediate {
    pub mode: Mode,
    pub language_candidates: Vec<String>,
    pub minimum_relative_distance: f64,
}

impl TryFrom<GetLangIntermediate> for GetLang {
    type Error = LibCfgError; // Use String as error type just for simplicity

    fn try_from(value: GetLangIntermediate) -> Result<Self, Self::Error> {
        let GetLangIntermediate {
            mode,
            language_candidates,
            minimum_relative_distance,
        } = value;

        #[cfg(feature = "lang-detection")]
        let language_candidates: Vec<IsoCode639_1> = language_candidates
            .iter()
            // No `to_uppercase()` required, this is done automatically by
            // `IsoCode639_1::from_str`.
            .map(|l| {
                IsoCode639_1::from_str(l.trim())
                    // Emit proper error message.
                    .map_err(|_| {
                        // The error path.
                        // Produce list of all available languages.
                        let mut all_langs = lingua::Language::all()
                            .iter()
                            .map(|l| {
                                let mut s = l.iso_code_639_1().to_string();
                                s.push_str(", ");
                                s
                            })
                            .collect::<Vec<String>>();
                        all_langs.sort();
                        let mut all_langs = all_langs.into_iter().collect::<String>();
                        all_langs.truncate(all_langs.len() - ", ".len());
                        // Insert data into error object.
                        LibCfgError::ParseLanguageCode {
                            language_code: l.into(),
                            all_langs,
                        }
                    })
            })
            .collect::<Result<Vec<IsoCode639_1>, LibCfgError>>()?;

        Ok(GetLang {
            mode,
            language_candidates,
            minimum_relative_distance,
        })
    }
}

#[derive(Default, Debug, Clone, PartialEq, Deserialize, Serialize)]
pub enum Mode {
    /// The `get_lang` filter is disabled. No language guessing occurs.
    Disabled,
    /// The algorithm of the `get_lang` filter assumes, that the input is
    /// monolingual. Only one language is searched and reported.
    Monolingual,
    /// The algorithm of the `get_lang` filter assumes, that the input is
    /// monolingual. If present in the input, more than one language can be
    /// reported.
    #[default]
    Multilingual,
    /// Variant to represent the error state for an invalid `GetLang` object.
    #[serde(skip)]
    Error(LibCfgError),
}

/// Configuration for the HTML exporter feature, deserialized from the
/// configuration file.
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct TmplHtml {
    pub viewer: String,
    pub viewer_error: String,
    pub viewer_doc_css: String,
    pub viewer_highlighting_theme: String,
    pub viewer_highlighting_css: String,
    pub exporter: String,
    pub exporter_doc_css: String,
    pub exporter_highlighting_theme: String,
    pub exporter_highlighting_css: String,
}

/// Defines the way the HTML exporter rewrites local links.
/// The command line option `--export-link-rewriting` expects this enum.
/// Consult the manpage for details.
#[derive(Debug, Hash, Clone, Eq, PartialEq, Deserialize, Serialize, Copy, Default)]
pub enum LocalLinkKind {
    /// Do not rewrite links.
    Off,
    /// Rewrite relative local links. Base: location of `.tpnote.toml`
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
pub enum Assertion {
    /// `IsDefined`: Assert that the variable is defined in the template.
    IsDefined,
    /// `IsNotEmptyString`: In addition to `IsString`, the condition asserts,
    /// that the string -or all substrings-) are not empty.
    IsNotEmptyString,
    /// `IsString`: Assert, that if the variable is defined, its type -or all
    /// subtypes- are `Value::String`.
    IsString,
    /// `IsNumber`: Assert, that if the variable is defined, its type -or all
    /// subtypes- are `Value::Number`.
    IsNumber,
    /// `IsBool`: Assert, that if the variable is defined, its type -or all
    /// subtypes- are `Value::Bool`.
    IsBool,
    /// `IsNotCompound`: Assert, that if the variable is defined, its type is
    /// not `Value::Array` or `Value::Object`.
    IsNotCompound,
    /// `IsValidSortTag`: Assert, that if the variable is defined, the value's
    /// string representation contains solely characters of the
    /// `filename.sort_tag.extra_chars` set, digits or lowercase letters.
    /// The number of lowercase letters in a row is limited by
    /// `tpnote_lib::config::FILENAME_SORT_TAG_LETTERS_IN_SUCCESSION_MAX`.
    IsValidSortTag,
    /// `IsConfiguredScheme`: Assert, that -if the variable is defined- the
    /// string equals to one of the `scheme.name` in the configuration file.
    IsConfiguredScheme,
    /// `IsTpnoteExtension`: Assert, that if the variable is defined,
    /// the values string representation is registered in one of the
    /// `filename.extension_*` configuration file variables.
    IsTpnoteExtension,
    /// `NoOperation` (default): A test that is always satisfied. For internal
    ///  use only.
    #[default]
    NoOperation,
}
