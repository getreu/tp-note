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

/// Maximum length of a note's filename in bytes. If a filename template produces
/// a longer string, it will be truncated.
pub const FILENAME_LEN_MAX: usize =
    // Most file system's limit.
    255
    // Additional separator.
    - 1
    // Additional copy counter.
    - FILENAME_COPY_COUNTER_OPENING_BRACKETS.len()
    - 2
    - FILENAME_COPY_COUNTER_CLOSING_BRACKETS.len()
    // Extra spare bytes, in case the user's copy counter is longer.
    - 6;

/// The apperance of a file with this filename marks the position of
/// `TMPL_VAR_ROOT_PATH`.
pub const FILENAME_ROOT_PATH_MARKER: &str = ".tpnote.toml";

/// List of characters that can be part of a _sort tag_.
/// This list must not include `SORT_TAG_EXTRA_SEPARATOR`.
/// The first character in the filename which is not
/// in this list, marks the end of the sort tag.
/// If `FILENAME_SORT_TAG_SEPARATOR` is not empty and the resulting string
/// terminates with `FILENAME_SORT_TAG_SEPARATOR` the latter is is stripped
/// from the result.
pub const FILENAME_SORT_TAG_CHARS: &str = "0123456789.-_ \t";

/// If empty, the first character which is not in `FILENAME_SORT_TAG_CHARS`
/// marks the end of a sort tag.
/// If not empty, a _sort_tag_ is only valid, when is it is followed by
/// `FILENAME_SORT_TAG_SEPARATOR`. A _sort_tag_ never ends with a
/// `FILENAME_SORT_TAG_SEPARATOR`, if it does it stripped. In other positions
/// the speparator may appear.
pub const FILENAME_SORT_TAG_SEPARATOR: &str = "-";

/// In case the file stem starts with a character in
/// `SORT_TAG_CHARS` the `SORT_TAG_EXTRA_SEPARATOR`
/// character is inserted in order to separate both parts
/// when the filename is read next time.
pub const FILENAME_SORT_TAG_EXTRA_SEPARATOR: char = '\'';

/// If the stem of a filename ends with a pattern, that is
/// similar to a copy counter, add this extra separator. It
/// must be one of `TRIM_LINE_CHARS` (see definition in
/// crate: `sanitize_filename_reader_friendly`) because they
/// are known not to appear at the end of `sanitze()`'d
/// strings. This is why they are suitable here.
pub const FILENAME_COPY_COUNTER_EXTRA_SEPARATOR: char = '-';

/// Tp-Note may add a counter at the end of the filename when
/// it can not save a file because the name is taken already.
/// This is the opening bracket search pattern. Some examples:
/// `"-"`, "'_'"", `"_-"`,`"-_"`, `"("`
/// Can be empty.
pub const FILENAME_COPY_COUNTER_OPENING_BRACKETS: &str = "(";

/// Tp-Note may add a counter at the end of the filename when
/// it can not save a file because the name is taken already.
/// This is the closing bracket search pattern. Some examples:
/// `"-"`, "'_'"", `"_-"`,`"-_"`, `"("`
/// Can be empty.
pub const FILENAME_COPY_COUNTER_CLOSING_BRACKETS: &str = ")";

/// When a filename is taken already, Tp-Note adds a copy
/// counter number in the range of `0..COPY_COUNTER_MAX`
/// at the end.
pub const FILENAME_COPY_COUNTER_MAX: usize = 400;

/// File extension of new _Tp-Note_ files.
///
/// For UNIX like systems this defaults to `.md` because all the
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
pub const FILENAME_EXTENSION_DEFAULT: &str = "md";

/// The variables `FILENAME_EXTENSIONS_*` list file extensions that Tp-Note
/// considers as its own note files. Tp-Note opens these files, reads their
/// YAML header and launches an external file editor and an file viewer (web
/// browser). According to the markup language used, the appropriate renderer
/// is called to convert the note's content into HTML. The rendered HTML is then
/// shown to the user with his web browser.
///
/// The present list contains file extensions of Markdown encoded Tp-Note files.
pub const FILENAME_EXTENSIONS_MD: &[&str] = &["txt", "md", "markdown", "markdn", "mdown", "mdtxt"];

/// The present list contains file extensions of RestructuredText encoded Tp-
/// Note files.
///
/// See also `FILENAME_EXTENSIONS_MD`.
pub const FILENAME_EXTENSIONS_RST: &[&str] = &["rst", "rest"];

/// The present list contains file extensions of HTML encoded Tp-Note files.
/// For these file types the content is forwarded to the web browser without
/// modification.
///
/// See also `FILENAME_EXTENSIONS_MD`.
pub const FILENAME_EXTENSIONS_HTML: &[&str] = &["htmlnote"];

/// The present list contains file extensions of Text encoded Tp-Note files
/// that the viewer shows literally without (almost) any additional rendering.
/// Only hyperlinks in _Markdown_, _reStructuredText_, _Asciidoc_ and _HTML_ are
/// rendered, thus clickable.
///
/// See also `FILENAME_EXTENSIONS_MD`.
pub const FILENAME_EXTENSIONS_TXT: &[&str] = &["txtnote", "adoc", "asciidoc", "mediawiki", "mw"];

/// The present list contains file extensions of Tp-Note files for which no
/// viewer is opened (unless Tp-Note is invoked with `--view`).
///
/// See also `FILENAME_EXTENSIONS_MD`.
pub const FILENAME_EXTENSIONS_NO_VIEWER: &[&str] = &["t2t"];

/// This a dot by definition.
pub const FILENAME_DOTFILE_MARKER: char = '.';

/// As all application logic is encoded in Tp-Note's templates, it does
/// not know about field names. Nevertheless, it is useful to identify at
/// least one field as _the_ field that identifies a note the most.  When
/// `TMPL_COMPULSORY_HEADER_FIELD` is not empty, Tp-Note will not synchronize
/// the note's filename and will pop up an error message, unless it finds the
/// field in the note's header.  When `TMPL_COMPULSORY_HEADER_FIELD` is empty,
/// all files are synchronized without any further field check. Make sure to
/// define a default value with `fm_* | default(value=*)` in case the variable
/// `fm_*` does not exist in the note's front matter.
const TMPL_COMPULSORY_HEADER_FIELD: &str = "title";

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

/// A list of language tags, defining languages TP-Note tries to recognize in
/// the filter input. The user's default language subtag, as reported from
/// the operating system, is automatically added to the present list.
/// The language recognition feature is disabled, when the list is empty.
/// It is also disabled, when the user's default language, as reported from
/// the operating system, is not supported by the external language guessing
/// library _Lingua_. In both cases the filter returns the empty string.
pub const TMPL_FILTER_GET_LANG: &[&str] = &["en", "fr", "de"];

/// A pseudo language tag for the `get_lang_filter`. When placed in the
/// `TMP_FILTER_GET_LANG` list, all available languages are selected.
pub const TMPL_FILTER_GET_LANG_ALL: &str = "+all";

/// Default values for the `map_lang` hash map filter, that is used to post
/// process the language recognition subtag as defined in `TMPL_GET_LANG`. The
/// key is the language subtag, the corresponding value adds a region subtag
/// completing the language tag. The default region subtags are chosen to be
/// compatible with the _LanguageTool_ grammar checker. In case a language
/// subtag has no key in the present hash map, the filter forwards the input
/// unchanged, e.g. the filter input `fr` results in `fr`.
/// One entry, derived from the user's default language - as reported from the
/// operating system - is automatically added to the present list. This
/// happens only when this language is not listed yet. For example,
/// consider the list `TMPL_FILTER_MAP_LANG = &[&["en", "en-US"]]`: In this
/// case, the user's default language `fr_CA.UTF-8` is added as
/// `&["fr", "fr-CA"]`. But, if the user's default language were
/// `en_GB.UTF-8`, then it is _not_ added because an entry `&["en", "en-US"]`
/// exists already.
/// Note,  that the empty input string results in the user's default language
/// tag - here `fr-CA` - as well.
pub const TMPL_FILTER_MAP_LANG: &[&[&str]] = &[&["de", "de-DE"], &["et", "et-ET"]];

/// Default value used by `to_yaml_filter`.
/// The parameter `TMPL_FILTER_TO_YAML_TAB_DEFAULT = n` indents the YAML values
/// `n` characters to the right of the first character of the key by inserting
/// additional spaces between the key and the value. `n==0` disables the
/// extra indentation.
pub const TMPL_FILTER_TO_YAML_TAB: u64 = 14;

/// Default content template used when the command line argument `<sanit>`
/// is a directory. Can be changed through editing the configuration
/// file. The following variables are  defined:
/// * `{{ path }}`: points to the directory where the new note will be
///   created.
/// * `{{ dir_path }}` is in this context identical to `{{ path }}`.
///  In addition, all environment variables can be used, e.g.
/// `{{ get_env(name=\"LOGNAME\") }}` When placed in YAML front matter, the
/// filter `to_yaml` must be appended to each variable.
pub const TMPL_NEW_CONTENT: &str = "\
{%- set title_text = dir_path | trim_file_sort_tag -%}
---
{{ title_text | cut | to_yaml(key='title') }}
{{ 'Note' | to_yaml(key='subtitle') }}
{{ username | capitalize | to_yaml(key='author') }}
{{ now() | date(format='%Y-%m-%d') | to_yaml(key='date') }}
{{ title_text | get_lang | map_lang(default=lang) | to_yaml(key='lang') }}
---


";

/// Default filename template for a new note file on disk. It implements the
/// sync criteria for note metadata in front matter and filename.
/// Useful variables in this context are:
/// `{{ title| sanit }}`, `{{ subtitle| sanit }}`, `{{ extension_default }}.
/// In general, in filename template, all variables (except `now` and
/// `extension_default` must be filtered by a `sanit` filter.
pub const TMPL_NEW_FILENAME: &str = "\
{%- set tag = now() | date(format='%Y%m%d') -%}
{{ fm_title | sanit | prepend(with_sort_tag=tag) }}\
{{ fm_subtitle | default(value='') | sanit | prepend(with='--') }}\
{{ extension_default | prepend(with='.') }}\
";

/// Default template used, when the clipboard or the input stream `stdin`
/// contains a string and one the of these strings contains a valid YAML front
/// matter section. The clipboards body is in `{{ clipboard }}`, the header
/// is in `{{ clipboard_header }}`.  The stdin's body is in `{{ stdin }}`,
/// the header is in `{{ stdin_header }}`. First all variables defined in the
/// clipboard's front matter are registered, the ones defined in the input
/// stream `stdin`. The latter can overwrite the former.  One of the front
/// matters must define the `title` variable, which is then available in this
/// template as `{{ fm_title }}`.
/// When placed in YAML front matter, the filter `to_yaml` must be
/// appended to each variable.
pub const TMPL_FROM_CLIPBOARD_YAML_CONTENT: &str = "\
---
{{ fm_title | default(value = path|trim_file_sort_tag) | cut | to_yaml(key='title') }}
{{ fm_subtitle | default(value = 'Note') | cut | to_yaml(key='subtitle') }}
{{ fm_author | default(value=username | capitalize) | to_yaml(key='author') }}
{{ fm_date | default(value = now()|date(format='%Y-%m-%d')) | to_yaml(key='date') }}
{{ fm_all|\
 field(out='fm_title')|\
 field(out='fm_subtitle')|\
 field(out='fm_author')|\
 field(out='fm_date')|\
 field(out='fm_lang')\
 | to_yaml | append(newline=true) }}\
{{ fm_lang | default(value = fm_title| \
                           default(value=stdin~clipboard|heading)| \
                 get_lang | map_lang(default=lang) ) | to_yaml(key='lang') }}
---

{{ stdin ~ clipboard | trim }}

";

/// Default filename template used when the stdin or the clipboard contains a
/// string and one of them has a valid YAML header.
pub const TMPL_FROM_CLIPBOARD_YAML_FILENAME: &str = "\
{%- set tag = fm_sort_tag | default(value = now() | date(format='%Y%m%d')) -%}
{{ fm_title | sanit | prepend(with_sort_tag=tag) }}\
{{ fm_subtitle | default(value='') | sanit | prepend(with='--') }}\
{{ fm_file_ext | default(value = extension_default ) | prepend(with='.') }}\
";

/// Default template used, when the clipboard or the input stream `stdin`
/// contains a string and this string has no valid YAML front matter section.
/// The clipboards content is in `{{ clipboard }}`, its truncated version in
/// `{{ clipboard | heading }}` When the clipboard contains a hyperlink in
/// Markdown or reStruncturedText format. See crate `parse-hyperlinks` for
/// details. For example: `[<link-name>](<link-url> "link-title")`, can be
/// accessed with the variables: `{{ clipboard | link_text }}`, `
/// {{ clipboard | link_dest }}` and `{{ clipboard | linkttitle }}`.
pub const TMPL_FROM_CLIPBOARD_CONTENT: &str = "\
{%- set lname = stdin ~ clipboard | link_text -%}
{%- set is_link_text =
        lname !='' and
        not lname is starting_with(\"http\")
        and not lname is starting_with(\"HTTP\") -%}
{%- if is_link_text -%}
    {%- set title_text = stdin ~ clipboard | link_text -%}
{%- else -%}
    {%- set title_text = stdin ~ clipboard | heading -%}
{% endif -%}
---
{{ title_text | cut | to_yaml(key='title') }}
{% if stdin ~ clipboard | link_text !='' and
      stdin ~ clipboard | cut | linebreaksbr == stdin ~ clipboard | cut -%}
  {{ 'URL' | to_yaml(key='subtitle') -}}
{%- else -%}
  {{ 'Note' | to_yaml(key='subtitle') -}}
{%- endif %}
{{ username | capitalize | to_yaml(key='author') }}
{{ now() | date(format='%Y-%m-%d') | to_yaml(key='date') }}
{{ title_text | get_lang | map_lang(default=lang) | to_yaml(key='lang') }}
---

{{ stdin ~ clipboard | trim }}

";

/// Default filename template used when the stdin ~ clipboard contains a string.
pub const TMPL_FROM_CLIPBOARD_FILENAME: &str = "\
{%- set tag = now() | date(format='%Y%m%d') -%}
{{ fm_title | sanit | prepend(with_sort_tag=tag) }}\
{{ fm_subtitle | default(value='') | sanit | prepend(with='--') }}\
{{ extension_default | prepend(with='.') }}";

/// Default template used, when the opened text file (with a known file
/// extension) is missing a YAML front matter section. This template prepends
/// such a header. The template inserts information extracted from the input
/// filename and its creation date. `{{ path }}` points to the text file,
/// `{{ dir_path }}` to the directory where it is located.
pub const TMPL_FROM_TEXT_FILE_CONTENT: &str = "\
---
{{ path | file_stem | split(pat='--') | first | cut | to_yaml(key='title') }}
{{ path | file_stem | split(pat='--') | nth(n=1) | cut | to_yaml(key='subtitle') }}
{{ username | capitalize | to_yaml(key='author') }}
{{ note_file_date | default(value='') | date(format='%Y-%m-%d') | \
   to_yaml(key='date') }}
{{ path | file_name | to_yaml(key='orig_name') }}
{{ note_body_text | get_lang | map_lang(default=lang) | to_yaml(key='lang') }}
---

{{ note_body_text }}
";

/// Default filename template used when the input file (with a known
/// file extension) is missing a YAML front matter section.
/// The text file's sort-tag and file extension are preserved.
pub const TMPL_FROM_TEXT_FILE_FILENAME: &str = "\
{%- if path | file_sort_tag != '' -%}
  {%- set tag = path | file_sort_tag -%}
{%- else -%}
  {%- set tag = note_file_date | date(format='%Y%m%d') -%}
{%- endif -%}
{{ fm_title | sanit | prepend(with_sort_tag=tag) }}\
{{ fm_subtitle | default(value='') | sanit | prepend(with='--') }}\
{{ path | file_ext | prepend(with='.') }}\
";

/// Default template used when the command line `<path>` parameter points to an
/// existing - to be annotated - non-`.md`-file. `{{ path}}` points to that
/// file, `{{ dir_path }}` to the directory where it is located.
pub const TMPL_ANNOTATE_FILE_CONTENT: &str = "\
{%- set body_text = stdin ~ clipboard | trim -%}
{%- if body_text != '' -%}
   {%- set lang_test_text = body_text | cut -%}
{%- else -%}
   {%- set lang_test_text = path | file_stem  -%}
{%- endif -%}
---
{{ path | trim_file_sort_tag | to_yaml(key='title') }}
{% if body_text | link_text !='' and
      body_text | heading == body_text -%}
{{ 'URL' | to_yaml(key='subtitle') -}}
{%- else -%}
{{ 'Note' | to_yaml(key='subtitle') -}}
{%- endif %}
{{ username | capitalize | to_yaml(key='author') }}
{{ now() | date(format='%Y-%m-%d') | to_yaml(key='date') }}
{{ lang_test_text | get_lang | map_lang(default=lang) | to_yaml(key='lang') }}
---

[{{ path | file_name }}](<{{ path | file_name }}>)
{% if body_text != '' -%}
{%- if body_text != body_text | heading %}
---
{% endif %}
{{ body_text }}
{% endif %}
";

/// Filename of a new note, that annotates an existing file on disk given in
/// `<path>`.
pub const TMPL_ANNOTATE_FILE_FILENAME: &str = "\
{%- set tag = path | file_sort_tag -%}
{{ fm_title | sanit | prepend(with_sort_tag=tag) }}\
{{ fm_subtitle | default(value='') | sanit | prepend(with='--') }}\
{{ extension_default | prepend(with='.') }}\
";

/// Default filename template to test, if the filename of an existing note file
/// on disk, corresponds to the note's meta data stored in its front matter. If
/// it is not the case, the note's filename will be renamed.
pub const TMPL_SYNC_FILENAME: &str = "\
{%- set tag = fm_sort_tag | default(value = path | file_sort_tag) -%}
{{ fm_title | default(value='No title') | sanit | prepend(with_sort_tag=tag) }}\
{{ fm_subtitle | default(value='') | sanit | prepend(with='--') }}\
{{ fm_file_ext | default(value = path | file_ext) | prepend(with='.') }}\
";

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

/// HTML template to render regular viewer pages.
/// We could set
/// `#[cfg(feature = "viewer")]`,
/// but we prefer the same config file structure independent
/// of the enabled features.
pub const TMPL_HTML_VIEWER: &str = r#"<!DOCTYPE html>
<html lang="{{ fm_lang | default(value='en') }}">
<head>
<meta charset="UTF-8">
<title>{{ fm_title }}</title>
<link rel="stylesheet" href="{{ note_css_path }}">
<style>
<!-- Customize the viewer CSS here -->
</style>
  </head>
  <body>
  <table class="fm">
    <tr>
    <th class="fmkey">title:</th>
    <th class="fmval"><b>{{ fm_title|to_html }}</b></th>
  </tr>
    <tr>
    <th class="fmkey">subtitle:</th>
    <th class="fmval">{{ fm_subtitle | default(value='')|to_html }}</th>
  </tr>
    <tr>
    <th class="fmkeygrey">date:</th>
    <th class="fmvalgrey">{{ fm_date | default(value='')|to_html }}</th>
  </tr>
  {% for k, v in fm_all| field(out='fm_title')|
                         field(out='fm_subtitle')|
                         field(out='fm_date')
  %}
    <tr>
    <th class="fmkeygrey">{{ k }}:</th>
    <th class="fmvalgrey">{{ v|to_html }}</th>
  </tr>
  {% endfor %}
  </table>
  <div class="note-body">{{ note_body_html }}</div>
  <script>{{ note_js }}</script>
</body>
</html>
"#;

/// HTML template to render the viewer-error page.
/// We could set
/// `#[cfg(feature = "viewer")]`,
/// but we prefer the same config file structure independent
/// of the enabled features.
pub const TMPL_HTML_VIEWER_ERROR: &str = r#"<!DOCTYPE html>
<html lang="en">
<head>
<meta charset="UTF-8">
<title>Syntax error</title>
<style>
.note-error { color: #523626; }
pre { white-space: pre-wrap; }
a { color: #316128; }
h1, h2, h3, h4, h5, h6 { color: #d3af2c; font-family:sans-serif; }
</style>
</head>
<body>
<h3>Syntax error</h3>
<p> in note file: <pre>{{ path }}</pre><p>
<div class="note-error">
<hr>
<pre>{{ note_error }}</pre>
<hr>
</div>
{{ note_erroneous_content_html }}
<script>{{ note_js }}</script>
</body>
</html>
"#;

/// HTML template used to render a note into html when the
/// rendition is saved to disk. Similar to `HTML_VIEWER_TMPL`
/// but does not inject JavaScript code.
pub const TMPL_HTML_EXPORTER: &str = r#"<!DOCTYPE html>
<html lang="{{ fm_lang | default(value='en') }}">
<head>
<meta charset="utf-8">
<title>{{ fm_title }}</title>
<style>
{{ note_css }}
<!-- Customize the exporter CSS here -->
</style>
  </head>
  <body>
  <table class="fm">
    <tr>
    <th class="fmkey">title:</th>
    <th class="fmval"><b>{{ fm_title|to_html }}</b></th>
  </tr>
    <tr>
    <th class="fmkey">subtitle:</th>
    <th class="fmval">{{ fm_subtitle | default(value='')|to_html }}</th>
  </tr>
    <tr>
    <th class="fmkeygrey">date:</th>
    <th class="fmvalgrey">{{ fm_date | default(value='')|to_html }}</th>
  </tr>
  {% for k, v in fm_all|
        field(out='fm_title')|
        field(out='fm_subtitle')|
        field(out='fm_date')
    %}
    <tr>
    <th class="fmkeygrey">{{ k }}:</th>
    <th class="fmvalgrey">{{ v|to_html }}</th>
  </tr>
  {% endfor %}
  </table>
  <div class="note-body">{{ note_body_html }}</div>
</body>
</html>
"#;

/// A constant holding common CSS code, used as embedded code in
/// the `TMPL_HTML_EXPORTER` template and as referenced code in the
/// `TMPL_HTML_VIEWER` template.
pub const TMPL_HTML_CSS_COMMON: &str = r#"/* Tp-Note's CSS */
table.fm {
  font-weight: normal;
  margin-left: auto;
  margin-right: auto;
  background-color: #f3f2e4;
  border:1px solid grey;
}
th.fmkey, th.fmkeygrey, th.fmval, th.fmvalgrey {
  font-weight: normal;
  padding-left:15px;
  padding-right:15px;
}
th.fmkey{ color:#444444; text-align:right; vertical-align:top;}
th.fmval{
  color:#316128;
  text-align:left;
  font-family:sans-serif;
}
th.fmkeygrey{ color:grey; text-align:right; vertical-align:top;}
th.fmvalgrey{ color:grey; text-align:left; }
ul.fm {
  padding-left: 15px;
  margin: 0px;
}
li.fm {
  padding-bottom: 0px;
}
blockquote.fm {
  margin: 0px;
  padding-left: 15px
}
pre { white-space: pre-wrap; }
em { color: #523626; }
a { color: #316128; }
h1 { font-size: 150% }
h2 { font-size: 132% }
h3 { font-size: 115% }
h4, h5, h6 { font-size: 100% }
h1, h2, h3, h4, h5, h6 { color: #263292; font-family:sans-serif; }
"#;

lazy_static! {
/// Global variable containing the filename and template related configuration
/// data.
    pub static ref LIB_CFG: RwLock<LibCfg> = RwLock::new(LibCfg::default());
}

/// Configuration data, deserialized from the configuration file.
#[derive(Debug, Serialize, Deserialize, Default)]
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
    pub root_path_marker: String,
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
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Tmpl {
    pub filter_get_lang: Vec<String>,
    pub filter_map_lang: Vec<Vec<String>>,
    pub filter_to_yaml_tab: u64,
    pub compulsory_header_field: String,
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
#[derive(Debug, Serialize, Deserialize, Clone)]
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

/// Default values for copy counter.
impl ::std::default::Default for Filename {
    fn default() -> Self {
        Filename {
            root_path_marker: FILENAME_ROOT_PATH_MARKER.to_string(),
            sort_tag_chars: FILENAME_SORT_TAG_CHARS.to_string(),
            sort_tag_separator: FILENAME_SORT_TAG_SEPARATOR.to_string(),
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

/// Default values for templates.
impl ::std::default::Default for Tmpl {
    fn default() -> Self {
        Tmpl {
            filter_get_lang: TMPL_FILTER_GET_LANG
                .iter()
                .map(|a| (*a).to_string())
                .collect(),
            filter_map_lang: TMPL_FILTER_MAP_LANG
                .iter()
                .map(|i| i.iter().map(|a| (*a).to_string()).collect())
                .collect(),
            filter_to_yaml_tab: TMPL_FILTER_TO_YAML_TAB,
            compulsory_header_field: TMPL_COMPULSORY_HEADER_FIELD.to_string(),
            new_content: TMPL_NEW_CONTENT.to_string(),
            new_filename: TMPL_NEW_FILENAME.to_string(),
            from_clipboard_yaml_content: TMPL_FROM_CLIPBOARD_YAML_CONTENT.to_string(),
            from_clipboard_yaml_filename: TMPL_FROM_CLIPBOARD_YAML_FILENAME.to_string(),
            from_clipboard_content: TMPL_FROM_CLIPBOARD_CONTENT.to_string(),
            from_clipboard_filename: TMPL_FROM_CLIPBOARD_FILENAME.to_string(),
            from_text_file_content: TMPL_FROM_TEXT_FILE_CONTENT.to_string(),
            from_text_file_filename: TMPL_FROM_TEXT_FILE_FILENAME.to_string(),
            annotate_file_content: TMPL_ANNOTATE_FILE_CONTENT.to_string(),
            annotate_file_filename: TMPL_ANNOTATE_FILE_FILENAME.to_string(),
            sync_filename: TMPL_SYNC_FILENAME.to_string(),
        }
    }
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

/// Default values for the exporter feature.
impl ::std::default::Default for TmplHtml {
    fn default() -> Self {
        TmplHtml {
            viewer: TMPL_HTML_VIEWER.to_string(),
            viewer_error: TMPL_HTML_VIEWER_ERROR.to_string(),
            exporter: TMPL_HTML_EXPORTER.to_string(),
            css: {
                let mut css = String::new();
                #[cfg(feature = "renderer")]
                css.push_str(&get_css());
                css.push_str(TMPL_HTML_CSS_COMMON);
                css
            },
        }
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
#[derive(Debug, Hash, Clone, Eq, PartialEq, Deserialize, Serialize, Copy)]
pub enum LocalLinkKind {
    /// Do not rewrite links.
    Off,
    /// Rewrite rel. local links. Base: ".tpnoteroot"
    Short,
    /// Rewrite all local links. Base: "/"
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
