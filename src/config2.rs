//! Set configuration defaults, reads and writes _Tp-Note_'s configuration file
//! and exposes the configuration as `static` variable.

use lazy_static::lazy_static;
use serde::{Deserialize, Serialize};
use std::sync::RwLock;

/// Maximum length of a note's filename in bytes. If a filename template produces
/// a longer string, it will be truncated.
#[cfg(not(test))]
pub const FILENAME_LEN_MAX: usize =
    // Most file system's limit.
    255
    // Additional separator.
    - 1
    // Additional copy counter.
    - FILENAME_COPY_COUNTER_OPENING_BRACKETS.len() - 2 - FILENAME_COPY_COUNTER_CLOSING_BRACKETS.len()
    // Extra spare bytes, in case the user's copy counter is longer.
    - 6;
#[cfg(test)]
pub const FILENAME_LEN_MAX: usize = 10;

/// List of characters that can be part of a _sort tag_.
/// This list must not include `SORT_TAG_EXTRA_SEPARATOR`.
/// The first character in the filename which is not
/// in this list, marks the end of the sort tag.
const FILENAME_SORT_TAG_CHARS: &str = "0123456789.-_ \t";

/// In case the file stem starts with a character in
/// `SORT_TAG_CHARS` the `SORT_TAG_EXTRA_SEPARATOR`
/// character is inserted in order to separate both parts
/// when the filename is read next time.
const FILENAME_SORT_TAG_EXTRA_SEPARATOR: char = '\'';

/// If the stem of a filename ends with a pattern, that is
/// similar to a copy counter, add this extra separator. It
/// must be one of `TRIM_LINE_CHARS` (see definition in
/// crate: `sanitize_filename_reader_friendly`) because they
/// are known not to appear at the end of `sanitze()`'d
/// strings. This is why they are suitable here.
const FILENAME_COPY_COUNTER_EXTRA_SEPARATOR: char = '-';

/// Tp-Note may add a counter at the end of the filename when
/// it can not save a file because the name is taken already.
/// This is the opening bracket search pattern. Some examples:
/// `"-"`, "'_'"", `"_-"`,`"-_"`, `"("`
/// Can be empty.
const FILENAME_COPY_COUNTER_OPENING_BRACKETS: &str = "(";

/// Tp-Note may add a counter at the end of the filename when
/// it can not save a file because the name is taken already.
/// This is the closing bracket search pattern. Some examples:
/// `"-"`, "'_'"", `"_-"`,`"-_"`, `"("`
/// Can be empty.
const FILENAME_COPY_COUNTER_CLOSING_BRACKETS: &str = ")";

/// When a filename is taken already, Tp-Note adds a copy
/// counter number in the range of `0..COPY_COUNTER_MAX`
/// at the end.
pub const FILENAME_COPY_COUNTER_MAX: usize = 400;

/// File extension of new _Tp-Note_ files.
///
/// For Unix-like systems this defaults to `.md` because all the
/// listed file editors (see `APP_ARGS_EDITOR`) support it. The
/// Windows default is `.txt` to ensure that the _Notepad_ editor can
/// handle these files properly.
///
/// As longs as all extensions are part of the same group, here
/// `FILENAME_EXTENSIONS_MD`, all note files are interpreted as
/// _Markdown_ on all systems.
///
/// NB: Do not forget to adapt the templates `TMPL_*` in case you set
/// this to another markup language.
#[cfg(all(target_family = "unix", not(target_vendor = "apple")))]
pub const FILENAME_EXTENSION_DEFAULT: &str = "md";
#[cfg(target_family = "windows")]
pub const FILENAME_EXTENSION_DEFAULT: &str = "txt";
#[cfg(all(target_family = "unix", target_vendor = "apple"))]
pub const FILENAME_EXTENSION_DEFAULT: &str = "md";

/// The variables `FILENAME_EXTENSIONSS_*` list file extensions that Tp-Note
/// considers as its own note files.
/// Tp-Note opens these files, reads their their YAML header and
/// launches an external file editor and an file viewer
/// (web browser).
/// According to the markup language used, the appropriate
/// renderer is called to convert the note's content into HTML.
/// The rendered HTML is then shown to the user with his
/// web browser.
///
/// The present list contains file extensions of
/// Markdown encoded Tp-Note files.
pub const FILENAME_EXTENSIONS_MD: &[&str] = &["txt", "md", "markdown", "markdn", "mdown", "mdtxt"];

/// The present list contains file extensions of
/// RestructuredText encoded Tp-Note files.
///
/// See also `FILENAME_EXTENSIONS_MD`.
pub const FILENAME_EXTENSIONS_RST: &[&str] = &["rst", "rest"];

/// The present list contains file extensions of
/// HTML encoded Tp-Note files. For these
/// file types their content is forwarded to the web browser
/// without modification.
///
/// See also `FILENAME_EXTENSIONS_MD`.
pub const FILENAME_EXTENSIONS_HTML: &[&str] = &["htmlnote"];

/// The present list contains file extensions of
/// Text encoded Tp-Note files that the viewer shows
/// literally without (almost) any additional rendering.
/// Only hyperlinks in _Markdown_, _reStructuredText_, _Asciidoc_ and _HTML_ are
/// rendered, thus clickable.
///
/// See also `FILENAME_EXTENSIONS_MD`.
pub const FILENAME_EXTENSIONS_TXT: &[&str] = &["txtnote", "adoc", "asciidoc", "mediawiki", "mw"];

/// The present list contains file extensions of
/// Tp-Note files for which no viewer is opened
/// (unless Tp-Note is invoked with `--view`).
///
/// See also `FILENAME_EXTENSIONS_MD`.
pub const FILENAME_EXTENSIONS_NO_VIEWER: &[&str] = &["t2t"];

/// This a dot by definition.
pub const FILENAME_DOTFILE_MARKER: char = '.';

lazy_static! {
/// Global variable containing the filename related configuration data.
    pub static ref CFG_FILENAME: RwLock<Filename> = RwLock::new(Filename::default());
}

/// Configuration of filename parsing, deserialized from the
/// configuration file.
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Filename {
    pub sort_tag_chars: String,
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

/// Default values for copy counter.
impl ::std::default::Default for Filename {
    fn default() -> Self {
        Filename {
            sort_tag_chars: FILENAME_SORT_TAG_CHARS.to_string(),
            sort_tag_extra_separator: FILENAME_SORT_TAG_EXTRA_SEPARATOR,
            copy_counter_extra_separator: FILENAME_COPY_COUNTER_EXTRA_SEPARATOR.to_string(),
            copy_counter_opening_brackets: FILENAME_COPY_COUNTER_OPENING_BRACKETS.to_string(),
            copy_counter_closing_brackets: FILENAME_COPY_COUNTER_CLOSING_BRACKETS.to_string(),
            extension_default: FILENAME_EXTENSION_DEFAULT.to_string(),
            extensions_md: FILENAME_EXTENSIONS_MD
                .iter()
                .map(|a| (*a).to_string())
                .collect(),
            extensions_rst: FILENAME_EXTENSIONS_RST
                .iter()
                .map(|a| (*a).to_string())
                .collect(),
            extensions_html: FILENAME_EXTENSIONS_HTML
                .iter()
                .map(|a| (*a).to_string())
                .collect(),
            extensions_txt: FILENAME_EXTENSIONS_TXT
                .iter()
                .map(|a| (*a).to_string())
                .collect(),
            extensions_no_viewer: FILENAME_EXTENSIONS_NO_VIEWER
                .iter()
                .map(|a| (*a).to_string())
                .collect(),
        }
    }
}
