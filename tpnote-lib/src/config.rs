//! Set configuration defaults, reads and writes _Tp-Note_'s configuration file
//! and exposes the configuration as `static` variable behind a mutex.
//! This makes it possible to modify all configuration defaults (and templates)
//! at runtime.
//!
//! ```rust
//! use tpnote_lib::config::LIB_CFG;
//!
//! let mut lib_cfg = LIB_CFG.write().unwrap();
//! (*lib_cfg).filename.copy_counter_extra_separator = '@'.to_string();
//! ```

use crate::error::ConfigError;
#[cfg(feature = "renderer")]
use crate::highlight::get_css;
use lazy_static::lazy_static;
#[cfg(feature = "lang-detection")]
use lingua::IsoCode639_1;
#[cfg(feature = "lang-detection")]
use serde::de::{self, Visitor};
#[cfg(feature = "lang-detection")]
use serde::Deserializer;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
#[cfg(feature = "lang-detection")]
use std::fmt;
use std::ops::Deref;
use std::{env, str::FromStr, sync::RwLock};

/// Name of the environment variable, that can be optionally
/// used to overwrite the user's default language setting.
/// This is used in various templates.
pub const ENV_VAR_TPNOTE_LANG: &str = "TPNOTE_LANG";

/// Name of the environment variable, that can be optionally
/// used to overwrite the user's login name.
/// This is used in various templates.
pub const ENV_VAR_TPNOTE_USER: &str = "TPNOTE_USER";

/// Maximum length of a note's filename in bytes. If a filename template produces
/// a longer string, it will be truncated.
pub const FILENAME_LEN_MAX: usize =
    // Most file system's limit.
    255
    // Additional separator.
    - 1
    // Additional copy counter.
    - FILENAME_COPY_COUNTER_OPENING_BRACKETS.len() - 2 - FILENAME_COPY_COUNTER_CLOSING_BRACKETS.len()
    // Extra spare bytes, in case the user's copy counter is longer.
    - 6;

/// The apperance of a file with this filename marks the position of
/// `TMPL_VAR_ROOT_PATH`.
pub const FILENAME_ROOT_PATH_MARKER: &str = ".tpnoteroot";

/// List of charnote_error_tera_templateacters that can be part of a _sort tag_.
/// This list must not include `SORT_TAG_EXTRA_SEPARATOR`.
/// The first character in the filename which is not
/// in this list, marks the end of the sort tag.
pub const FILENAME_SORT_TAG_CHARS: &str = "0123456789.-_ \t";

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
#[cfg(all(target_family = "unix", not(target_vendor = "apple")))]
pub const FILENAME_EXTENSION_DEFAULT: &str = "md";
#[cfg(target_family = "windows")]
pub const FILENAME_EXTENSION_DEFAULT: &str = "txt";
#[cfg(all(target_family = "unix", target_vendor = "apple"))]
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
/// not know about field names. Nevertheless it is useful to identify at
/// least one field as _the_ field that identifies a note the most.  When
/// `TMPL_COMPULSORY_HEADER_FIELD` is not empty, Tp-Note will not synchronize
/// the note's filename and will pop up an error message, unless it finds the
/// field in the note's header.  When `TMPL_COMPULSORY_HEADER_FIELD` is empty,
/// all files are synchronized without any further field check. Make sure to
/// define a default value with `fm_* | default(value=*)` in case the variable
/// `fm_*` does not exist in the note's front matter.
const TMPL_COMPULSORY_HEADER_FIELD: &str = "title";

/// The template variable contains the fully qualified path of the `<path>`
/// command line argument. If `<path>` points to a file, the variable contains the
/// file path. If it points to a directory, it contains the directory path, or -
/// if no `path` is given - the current working directory.
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
/// of the current note. As all front matter variables, it's value is copied as
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
/// `filename_sync: false`, the filename synchronisation mechanism is
/// disabled for this note file. Default value is `true`.
pub const TMPL_VAR_FM_FILENAME_SYNC: &str = "fm_filename_sync";

/// A list of language tags, defining languages TP-Note tries to recognize in
/// the filter input. The user's default language subtag, as reported from
/// the operating system, is automatically added to the present list.
/// The language recognition feature is disabled, when the list is empty.
/// It is also disabled, when the user's default language, as reported from
/// the operating system, is not supported by the external language guessing
/// library _Lingua_. In both cases the filter returns the empty string.
#[cfg(feature = "lang-detection")]
pub const TMPL_FILTER_GET_LANG: &[DetectableLanguage<IsoCode639_1>] = &[
    DetectableLanguage(IsoCode639_1::EN),
    DetectableLanguage(IsoCode639_1::FR),
    DetectableLanguage(IsoCode639_1::DE),
    DetectableLanguage(IsoCode639_1::ET),
];

/// Ignored placeholder when the feature `lang-detection` is disabled.
#[cfg(not(feature = "lang-detection"))]
pub const TMPL_FILTER_GET_LANG: &[DetectableLanguage<String>] = &[];

/// Default values for the `map_lang` hash map filter, that is used to post
/// process the language recognition subtag as defined in `TMPL_GET_LANG`. The
/// key is the language subtag, the corresponding value adds a region subtag
/// completing the language tag. The default region subtags are chosen to be
/// compatible with the _LanguageTool_ grammar checker. In case a language
/// subtag has no key in the present hash map, the filter forward the input
/// unchanged, e.g. the filter input `fr` results in `fr`.
/// One entry, derived from the user's default language - as reported from the
/// operating system - is automatically added to the present list. For example,
/// the user's default language `fr_CA.UTF-8` is added as `&["fr", "fr-CA"]`.
/// Note that, the empty input string results in the user's default language
/// tag - here `fr-CA` - as well.
pub const TMPL_FILTER_MAP_LANG: &[&[&str]] =
    &[&["de", "de-DE"], &["et", "et-ET"]];

/// Default content template used when the command line argument `<sanit>`
/// is a directory. Can be changed through editing the configuration
/// file. The following variables are  defined: `{{ sanit | stem }}
/// `, `{{ path | stem }}`, `{{ path | ext }}`, `{{ extension_default }}`
/// `{{ file | tag }}`, `{{ username }}`, `{{ date }}`,
/// `{{ title_text | lang }}`, `{{ dir_path }}`. In addition all environment
/// variables can be used, e.g. `{{ get_env(name=\"LOGNAME\") }}` When placed
/// in YAML front matter, the filter `| json_encode` must be appended to each
/// variable.
pub const TMPL_NEW_CONTENT: &str = "\
{%- set title_text = dir_path | trim_tag -%}
---
title:      {{ title_text | cut | json_encode }}
subtitle:   {{ 'Note' | json_encode }}
author:     {{ username | capitalize | json_encode }}
date:       {{ now() | date(format='%Y-%m-%d') | json_encode }}
lang:       {{ title_text | get_lang | map_lang | json_encode }}
---


";

/// Default filename template for a new note file on disk. It implements the
/// sync criteria for note metadata in front matter and filename.
/// Useful variables in this context are:
/// `{{ title| sanit }}`, `{{ subtitle| sanit }}`, `{{ extension_default }}
/// `, All variables also exist in a `{{ <var>| sanit(alpha) }}` variant: in
/// case its value starts with a number, the string is prepended with `'`.
/// The first non-numerical variable must be some `{{ <var>| sanit(alpha) }}
/// ` variant. Note, as this is filename template, all variables (except
/// `now` and `extension_default` must be filtered by a `sanit` or
/// `sanit(force_alpha=true)` filter.
pub const TMPL_NEW_FILENAME: &str = "\
{{ now() | date(format='%Y%m%d-') }}\
{{ fm_title | sanit(force_alpha=true) }}{% if fm_subtitle | default(value='') | sanit != '' %}--{% endif %}\
{{ fm_subtitle | default(value='') | sanit  }}{{ extension_default | prepend_dot }}\
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
/// When placed in YAML front matter, the filter `| json_encode` must be
/// appended to each variable.
pub const TMPL_FROM_CLIPBOARD_YAML_CONTENT: &str = "\
---
title:      {{ fm_title | default(value = path|trim_tag) | cut | json_encode }}
subtitle:   {{ fm_subtitle | default(value = 'Note') | cut | json_encode }}
author:     {{ fm_author | default(value=username | capitalize) | json_encode }}
date:       {{ fm_date | default(value = now()|date(format='%Y-%m-%d')) | json_encode }}
{% for k, v in fm_all|\
 remove(var='fm_title')|\
 remove(var='fm_subtitle')|\
 remove(var='fm_author')|\
 remove(var='fm_date')|\
 remove(var='fm_lang')\
 %}{{ k }}:\t\t{{ v | json_encode }}
{% endfor -%}
lang:       {{ fm_lang | default(value = fm_title|\
                           default(value=stdin~clipboard|heading)|\
                 get_lang | map_lang ) | json_encode }}
---

{{ stdin ~ clipboard }}

";

/// Default filename template used when the stdin or the clipboard contains a
/// string and one of them has a valid YAML header.
pub const TMPL_FROM_CLIPBOARD_YAML_FILENAME: &str = "\
{{ fm_sort_tag | default(value = now() | date(format='%Y%m%d-')) }}\
{{ fm_title | sanit(force_alpha=true) }}\
{% if fm_subtitle | default(value='') | sanit != '' %}--{% endif %}\
{{ fm_subtitle | default(value='') | sanit  }}\
{{ fm_file_ext | default(value = extension_default ) | prepend_dot }}\
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
{%- set is_link_text = lname !=''and not lname is starting_with(\"http\")and not lname is starting_with(\"HTTP\") -%}
{%- if is_link_text %}{% set title_text = stdin ~ clipboard | link_text %}{% else %}{% set title_text = stdin ~ clipboard | heading %}{% endif -%}
---
title:      {{ title_text | cut | json_encode }}
{% if stdin ~ clipboard | link_text !='' and stdin ~ clipboard | cut | linebreaksbr == stdin ~ clipboard | cut %}subtitle:   {{ 'URL' | json_encode }}
{% else %}subtitle:   {{ 'Note' | json_encode }}
{% endif %}author:     {{ username | capitalize | json_encode }}
date:       {{ now() | date(format='%Y-%m-%d') | json_encode }}
lang:       {{ title_text | get_lang | map_lang | json_encode }}
---

{{ stdin ~ clipboard }}

";

/// Default filename template used when the stdin ~ clipboard contains a string.
pub const TMPL_FROM_CLIPBOARD_FILENAME: &str = "\
{{ now() | date(format='%Y%m%d-') }}\
{{ fm_title | sanit(force_alpha=true) }}\
{% if fm_subtitle | default(value='') | sanit != '' %}--{% endif %}\
{{ fm_subtitle | default(value='') | sanit  }}{{ extension_default | prepend_dot }}\
";

/// Default template used, when the opened text file (with a known file
/// extension) is missing a YAML front matter section. This template prepends
/// such a section. The template inserts information extracted from the input
/// filename and its creation date.
pub const TMPL_FROM_TEXT_FILE_CONTENT: &str = "\
---
title:      {{ path | stem | split(pat='--') | first | cut | json_encode }}
subtitle:   {{ path | stem | split(pat='--') | nth(n=1) | cut | json_encode }}
author:     {{ username | capitalize | json_encode }}
date:       {{ note_file_date | default(value='') | date(format='%Y-%m-%d') | json_encode }}
orig_name:  {{ path | filename | json_encode }}
lang:       {{ note_body_text | get_lang | map_lang | json_encode }}
---

{{ note_body_text }}
";

/// Default filename template used when the input file (with a known
/// file extension) is missing a YAML front matter section.
/// The text file's sort-tag and file extension are preserved.
pub const TMPL_FROM_TEXT_FILE_FILENAME: &str = "\
{% if path | tag == '' %}{{ note_file_date | date(format='%Y%m%d-') }}\
{% else %}{{ path | tag }}{% endif %}\
{{ fm_title | sanit(force_alpha=true) }}\
{% if fm_subtitle | default(value='') | sanit != '' %}--{% endif %}\
{{ fm_subtitle | default(value='') | sanit  }}\
{{ path | ext | prepend_dot }}\
";

/// Default template used when the command line `<path>` parameter points to an
/// existing non-`.md`-file. Can be modified through editing the configuration
/// file.
pub const TMPL_ANNOTATE_FILE_CONTENT: &str = "\
{%- set title_text = path | trim_tag -%}
---
title:      {{ title_text | json_encode }}
{% if stdin ~ clipboard | link_text !='' and stdin ~ clipboard | heading == stdin ~ clipboard %}subtitle:   {{ 'URL' | json_encode }}
{% else %}subtitle:   {{ 'Note' | json_encode }}
{% endif %}author:     {{ username | capitalize | json_encode }}
date:       {{ now() | date(format='%Y-%m-%d') | json_encode }}
lang:       {{ title_text | get_lang | map_lang | json_encode }}
---

[{{ path | filename }}](<{{ path | filename }}>)
{% if stdin ~ clipboard != '' %}{% if stdin ~ clipboard != stdin ~ clipboard | heading %}
---
{% endif %}
{{ stdin ~ clipboard }}
{% endif %}
";

/// Filename of a new note, that annotates an existing file on disk given in
/// `<path>`.
pub const TMPL_ANNOTATE_FILE_FILENAME: &str = "\
{{ path | tag }}{{ fm_title | sanit(force_alpha=true) }}\
{% if fm_subtitle | default(value='') | sanit != '' %}--{% endif %}\
{{ fm_subtitle | default(value='') | sanit }}{{ extension_default | prepend_dot }}\
";

/// Default filename template to test, if the filename of an existing note file
/// on disk, corresponds to the note's meta data stored in its front matter. If
/// it is not the case, the note's filename will be renamed.  Can be modified
/// through editing the configuration file.
pub const TMPL_SYNC_FILENAME: &str = "\
{{ fm_sort_tag | default(value = path | tag) }}\
{{ fm_title | default(value='No title') | sanit(force_alpha=true) }}\
{% if fm_subtitle | default(value='') | sanit != '' %}--{% endif %}\
{{ fm_subtitle | default(value='') | sanit  }}\
{{ fm_file_ext | default(value = path | ext) | prepend_dot }}\
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
  <table class="center">
    <tr>
    <th class="key">title:</th>
    <th class="val"><b>{{ fm_title }}</b></th>
  </tr>
    <tr>
    <th class="key">subtitle:</th>
    <th class="val">{{ fm_subtitle | default(value='') }}</th>
  </tr>
    <tr>
    <th class="keygrey">date:</th>
    <th class="valgrey">{{ fm_date | default(value='') }}</th>
  </tr>
  {% for k, v in fm_all| remove(var='fm_title')| remove(var='fm_subtitle')| remove(var='fm_date') %}
    <tr>
    <th class="keygrey">{{ k }}:</th>
    <th class="valgrey">{{ v }}</th>
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
  <table class="center">
    <tr>
    <th class="key">title:</th>
    <th class="val"><b>{{ fm_title }}</b></th>
  </tr>
    <tr>
    <th class="key">subtitle:</th>
    <th class="val">{{ fm_subtitle | default(value='') }}</th>
  </tr>
    <tr>
    <th class="keygrey">date:</th>
    <th class="valgrey">{{ fm_date | default(value='') }}</th>
  </tr>
  {% for k, v in fm_all| remove(var='fm_title')| remove(var='fm_subtitle')| remove(var='fm_date') %}
    <tr>
    <th class="keygrey">{{ k }}:</th>
    <th class="valgrey">{{ v }}</th>
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
table, th, td { font-weight: normal; }
table.center {
  margin-left: auto;
  margin-right: auto;
  background-color: #f3f2e4;
  border:1px solid grey;
}
th, td {
  padding: 3px;
  padding-left:15px;
  padding-right:15px;
}
th.key{ color:#444444; text-align:right; }
th.val{
  color:#316128;
  text-align:left;
  font-family:sans-serif;
}
th.keygrey{ color:grey; text-align:right; }
th.valgrey{ color:grey; text-align:left; }
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
/// Global variable containing the user's language tag from the `LANG`
/// environment variable (UNIX) or from the operation system (Windows).
    pub static ref LANG: String = {
        // Get the user's language tag.
        // [RFC 5646, Tags for the Identification of Languages](http://www.rfc-editor.org/rfc/rfc5646.txt)
        let mut lang;
        // Get the environment variable if it exists.
        let tpnotelang = env::var(ENV_VAR_TPNOTE_LANG).ok();
        // Unix/MacOS version.
        #[cfg(not(target_family = "windows"))]
        if let Some(tpnotelang) = tpnotelang {
            lang = tpnotelang;
        } else {
            // [Linux: Define Locale and Language Settings -
            // ShellHacks](https://www.shellhacks.com/linux-define-locale-language-settings/)
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
            lang = language.to_string();
            lang.push('-');
            lang.push_str(territory);
        }

        // Get the user's language tag.
        // Windows version.
        #[cfg(target_family = "windows")]
        if let Some(tpnotelang) = tpnotelang {
            lang = tpnotelang;
        } else {
            let mut buf = [0u16; LOCALE_NAME_MAX_LENGTH as usize];
            let len = unsafe { GetUserDefaultLocaleName(buf.as_mut_ptr(), buf.len() as i32) };
            if len > 0 {
                lang = String::from_utf16_lossy(&buf[..((len - 1) as usize)]);
            }
        };

        // Return value.
        lang
    };
}

lazy_static! {
/// Global variable containing the filename related configuration data.
    pub static ref LIB_CFG: RwLock<LibCfg> = RwLock::new(LibCfg::default());
}

/// Configuration data, deserialized from the configuration file.
#[derive(Debug, Serialize, Deserialize, Default)]
pub struct LibCfg {
    /// Version number of the config file as String -or-
    /// a text message explaining why we could not load the
    /// configuration file.
    pub filename: Filename,
    pub tmpl: Tmpl,
    pub tmpl_html: TmplHtml,
}

/// Configuration of filename parsing, deserialized from the
/// configuration file.
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Filename {
    pub root_path_marker: String,
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

/// Filename templates and content templates, deserialized from the
/// configuration file.
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Tmpl {
    #[cfg(feature = "lang-detection")]
    pub filter_get_lang: Vec<DetectableLanguage<IsoCode639_1>>,
    #[cfg(not(feature = "lang-detection"))]
    pub filter_get_lang: Vec<DetectableLanguage<String>>,
    pub filter_map_lang: Vec<Vec<String>>,
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
            filter_get_lang: TMPL_FILTER_GET_LANG.iter().map(|l| l.to_owned()).collect(),
            filter_map_lang: TMPL_FILTER_MAP_LANG
                .iter()
                .map(|i| i.iter().map(|a| (*a).to_string()).collect())
                .collect(),
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

lazy_static! {
    /// Store the extension as key and mime type as value in HashMap.
    pub(crate) static ref TMP_FILTER_MAP_LANG_HMAP: HashMap<String, String> = {
        let mut hm = HashMap::new();
        let lib_cfg = LIB_CFG.read().unwrap();
        for l in &lib_cfg.tmpl.filter_map_lang {
            if l.len() >= 2 {
                hm.insert(l[0].to_string(), l[1].to_string());
            };
        };
        // Insert the user's default language and region in hashmap.
        if let Some((lang_subtag, _)) = &LANG.split_once('-'){
            hm.insert(lang_subtag.to_string(), LANG.to_string() );
        };
        hm
    };
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
    type Err = ConfigError;
    fn from_str(level: &str) -> Result<LocalLinkKind, Self::Err> {
        match &*level.to_ascii_lowercase() {
            "off" => Ok(LocalLinkKind::Off),
            "short" => Ok(LocalLinkKind::Short),
            "long" => Ok(LocalLinkKind::Long),
            _ => Err(ConfigError::ParseLocalLinkKind {}),
        }
    }
}

/// A wrapper around the `IsoCode639_1` type which enables us to implement
/// the missing `Clone`, `Serialize` and `Deserialize` traits here.
#[cfg(feature = "lang-detection")]
#[derive(Debug, Eq, PartialEq)]
pub struct DetectableLanguage<T>(pub T);

/// A wrapper type around `&str`, which is used as a placeholder
/// When the `IsoCode639` was not pulled in.
#[cfg(not(feature = "lang-detection"))]
#[derive(Debug, Eq, PartialEq, Clone, Serialize, Deserialize)]
pub struct DetectableLanguage<T>(pub T);

impl<T> Deref for DetectableLanguage<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

#[cfg(feature = "lang-detection")]
impl Serialize for DetectableLanguage<IsoCode639_1> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        format!("{}", &self.0).serialize(serializer)
    }
}

/// Helper type required when implementing `Deserialize` for `IsoCode639`.
#[cfg(feature = "lang-detection")]
struct DetectableLanguageVisitor;

#[cfg(feature = "lang-detection")]
impl<'de> Visitor<'de> for DetectableLanguageVisitor {
    type Value = DetectableLanguage<IsoCode639_1>;

    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter.write_str("an IsoCode639_1 supported by the Lingua crate")
    }

    fn visit_str<E>(self, s: &str) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        match IsoCode639_1::from_str(s) {
            Ok(iso) => Ok(DetectableLanguage(iso)),
            Err(e) => Err(E::custom(format!(
                "ISO 639-1 language code is not supported by the Lingua crate: {}",
                e
            ))),
        }
    }
}

#[cfg(feature = "lang-detection")]
impl<'de> Deserialize<'de> for DetectableLanguage<IsoCode639_1> {
    fn deserialize<D>(deserializer: D) -> Result<DetectableLanguage<IsoCode639_1>, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_i32(DetectableLanguageVisitor)
    }
}

#[cfg(feature = "lang-detection")]
impl Clone for DetectableLanguage<IsoCode639_1> {
    fn clone(&self) -> Self {
        Self(IsoCode639_1::from_str(&format!("{}", &self.0)).unwrap())
    }
}
