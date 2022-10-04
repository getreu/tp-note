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

/// As all application logic is encoded in Tp-Note's templates, it does not know about field names.
/// Nevertheless it is useful to identify at least one field as _the_ field that identifies a note
/// the most.  When `TMPL_COMPULSORY_HEADER_FIELD` is not empty, Tp-Note will not synchronize the
/// note's filename and will pop up an error message, unless it finds the field in the note's
/// header.  When `TMPL_COMPULSORY_HEADER_FIELD` is empty, all files are synchronized without any
/// further field check. Make sure to define a default value with `fm_* | default(value=*)`
/// in case the variable `fm_*` does not exist in the note's front matter.
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
/// If defined, the environment variable `TPNOTELANG` overwrites this value
/// (all operating systems).
pub const TMPL_VAR_LANG: &str = "lang";

///  Contains the body of the file the command line option `<path>`
///  points to. Only available in the `TMPL_FROM_TEXT_FILE_CONTENT` template.
///  Only available in the `TMPL_FROM_TEXT_FILE_CONTENT` template.
pub const TMPL_VAR_PATH_FILE_TEXT: &str = "path_file_text";

///  Contains the date of the file the command line option `<path>` points to.
///  The date is represented as an integer the way `std::time::SystemTime`
///  resolves to on the platform. Only available in the
///  `TMPL_FROM_TEXT_FILE_CONTENT` template.
pub const TMPL_VAR_PATH_FILE_DATE: &str = "path_file_date";

/// Prefix prepended to front matter field names when a template variable
/// is generated with the same name.
pub const TMPL_VAR_FM_: &str = "fm_";

/// Contains a Hash Map with all front matter fields. Lists are flattened
/// into a strings.
pub const TMPL_VAR_FM_ALL: &str = "fm_all";

/// All the front matter fields serialized as text, exactly as they appear in
/// the front matter.
pub const TMPL_VAR_FM_ALL_YAML: &str = "fm_all_yaml";

/// By default, the template `TMPL_SYNC_FILENAME` defines the function of
/// of this variable as follows:
/// Contains the value of the front matter field `file_ext` and determines the
/// markup language used to render the document. When the field is missing the
/// markup language is derived from the note's filename extension.
///
/// This is a dynamically generated variable originating from the front matter
/// of the current note. As all front matter variables, it's value is copied as
/// it is without modification.  Here, the only special treatment is, when
/// analyzing the front matter, it is verified, that the value of this variable
/// is registered in one of the `[filename] extensions_*` variables.
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
/// value of this variable are listed in `[filename] sort_tag_chars`.
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
/// Default content template used when the command line argument <sanit> is a directory. Can be

/// changed through editing the configuration file.
/// The following variables are  defined:
/// `{{ sanit | stem }}`, `{{ path | stem }}`, `{{ path | ext }}`, `{{ extension_default }}` `{{
/// file | tag }}`, `{{ username }}`, `{{ date }}`, `{{ lang }}`, `{{ dir_path }}`.
/// In addition all environment variables can be used, e.g.  `{{ get_env(name=\"LOGNAME\") }}`
/// When placed in YAML front matter, the filter `| json_encode` must be appended to each variable.
pub const TMPL_NEW_CONTENT: &str = "\
---
title:      {{ dir_path | trim_tag | cut | json_encode }}
subtitle:   {{ 'Note' | json_encode }}
author:     {{ username | json_encode }}
date:       {{ now() | date(format='%Y-%m-%d') | json_encode }}
lang:       {{ lang | json_encode }}
---


";

/// Default filename template for a new note file on disk. It implements the sync criteria for
/// note metadata in front matter and filename.
/// Useful variables in this context are:
/// `{{ title| sanit }}`, `{{ subtitle| sanit }}`, `{{ extension_default }}`,
/// All variables also exist in a `{{ <var>| sanit(alpha) }}` variant: in case its value starts
/// with a number, the string is prepended with `'`.  The first non-numerical variable must be some
/// `{{ <var>| sanit(alpha) }}` variant.
/// Note, as this is filename template, all variables (except `now` and `extension_default` must be
/// filtered by a `sanit` or `sanit(force_alpha=true)` filter.
pub const TMPL_NEW_FILENAME: &str = "\
{{ now() | date(format='%Y%m%d-') }}\
{{ fm_title | sanit(force_alpha=true) }}{% if fm_subtitle | default(value='') | sanit != '' %}--{% endif %}\
{{ fm_subtitle | default(value='') | sanit  }}{{ extension_default | prepend_dot }}\
";

/// Default template used, when the clipboard or the input stream `stdin` contains a string and one
/// the of these strings contains a valid YAML front matter section.
/// The clipboards body is in `{{ clipboard }}`, the header is in `{{ clipboard_header }}`.  The
/// stdin's body is in `{{ stdin }}`, the header is in `{{ stdin_header }}`.
/// First all variables defined in the clipboard's front matter are registered, the ones
/// defined in the input stream `stdin`. The latter can overwrite the former.  One of the front
/// matters must define the `title` variable, which is then available in this template as `{{
/// fm_title }}`.
/// When placed in YAML front matter, the filter `| json_encode` must be
/// appended to each variable.
pub const TMPL_FROM_CLIPBOARD_YAML_CONTENT: &str = "\
---
title:      {{ fm_title | default(value = path|trim_tag) | cut | json_encode }}
subtitle:   {{ fm_subtitle | default(value = 'Note') | cut | json_encode }}
author:     {{ fm_author | default(value=username) | json_encode }}
date:       {{ fm_date | default(value = now()|date(format='%Y-%m-%d')) | json_encode }}
lang:       {{ fm_lang | default(value = lang) | json_encode }}
{% for k, v in fm_all\
 | remove(var='fm_title')\
 | remove(var='fm_subtitle')\
 | remove(var='fm_author')\
 | remove(var='fm_date')\
 | remove(var='fm_lang') %}\
{{ k }}:\t\t{{ v | json_encode }}
{% endfor %}\
---

{{ stdin ~ clipboard }}

";

/// Default filename template used when the stdin or the clipboard contains a string and one of
/// them has a valid YAML header.
pub const TMPL_FROM_CLIPBOARD_YAML_FILENAME: &str = "\
{{ fm_sort_tag | default(value = now() | date(format='%Y%m%d-')) }}\
{{ fm_title | sanit(force_alpha=true) }}\
{% if fm_subtitle | default(value='') | sanit != '' %}--{% endif %}\
{{ fm_subtitle | default(value='') | sanit  }}\
{{ fm_file_ext | default(value = extension_default ) | prepend_dot }}\
";

/// Default template used, when the clipboard or the input stream `stdin` contains a string and
/// this string has no valid YAML front matter section.  The clipboards content is in `{{ clipboard
/// }}`, its truncated version in `{{ clipboard | heading }}` When the clipboard contains a
/// hyperlink in Markdown or reStruncturedText format. See crate `parse-hyperlinks` for details.
/// For example: `[<link-name>](<link-url> "link-title")`, can be accessed with the variables:
/// `{{ clipboard | linkname }}`, `{{ clipboard | linktarget }}` and `{{ clipboard | linkttitle }}`.
pub const TMPL_FROM_CLIPBOARD_CONTENT: &str = "\
{%- set lname = stdin ~ clipboard | linkname -%}
{%- set ok_linkname = lname !=''\
    and not lname is starting_with(\"http\")\
    and not lname is starting_with(\"HTTP\") -%}
---
{% if ok_linkname %}\
title:      {{ stdin ~ clipboard | linkname | cut | json_encode }}
{% else %}\
title:      {{ stdin ~ clipboard | heading | cut | json_encode }}
{% endif %}\
{% if stdin ~ clipboard | linkname !='' and stdin ~ clipboard | cut | linebreaksbr == stdin ~ clipboard | cut %}\
subtitle:   {{ 'URL' | json_encode }}
{% else %}\
subtitle:   {{ 'Note' | json_encode }}
{% endif %}\
author:     {{ username | json_encode }}
date:       {{ now() | date(format='%Y-%m-%d') | json_encode }}
lang:       {{ lang | json_encode }}
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
author:     {{ username | json_encode }}
date:       {{ path_file_date | date(format='%Y-%m-%d') | json_encode }}
orig_name:  {{ path | filename | json_encode }}
lang:       {{ lang | json_encode }}
---

{{ path_file_text }}
";

/// Default filename template used when the input file (with a known
/// file extension) is missing a YAML front matter section.
/// The text file's sort-tag and file extension are preserved.
pub const TMPL_FROM_TEXT_FILE_FILENAME: &str = "\
{% if path | tag == '' %}{{ path_file_date | date(format='%Y%m%d-') }}\
{% else %}{{ path | tag }}{% endif %}\
{{ fm_title | sanit(force_alpha=true) }}\
{% if fm_subtitle | default(value='') | sanit != '' %}--{% endif %}\
{{ fm_subtitle | default(value='') | sanit  }}\
{{ path | ext | prepend_dot }}\
";

/// Default template used when the command line <path> parameter points to an existing
/// non-`.md`-file. Can be modified through editing the configuration file.
pub const TMPL_ANNOTATE_FILE_CONTENT: &str = "\
---
title:      {{ path | trim_tag | json_encode }}
{% if stdin ~ clipboard | linkname !='' and stdin ~ clipboard | heading == stdin ~ clipboard %}\
subtitle:   {{ 'URL' | json_encode }}
{% else %}\
subtitle:   {{ 'Note' | json_encode }}
{% endif %}\
author:     {{ username | json_encode }}
date:       {{ now() | date(format='%Y-%m-%d') | json_encode }}
lang:       {{ lang | json_encode }}
---

[{{ path | filename }}](<{{ path | filename }}>)
{% if stdin ~ clipboard != '' %}{% if stdin ~ clipboard != stdin ~ clipboard | heading %}
---
{% endif %}
{{ stdin ~ clipboard }}
{% endif %}
";

/// Filename of a new note, that annotates an existing file on disk given in
/// <path>.
pub const TMPL_ANNOTATE_FILE_FILENAME: &str = "\
{{ path | tag }}{{ fm_title | sanit(force_alpha=true) }}\
{% if fm_subtitle | default(value='') | sanit != '' %}--{% endif %}\
{{ fm_subtitle | default(value='') | sanit }}{{ extension_default | prepend_dot }}\
";

/// Default filename template to test, if the filename of an existing note file on disk,
/// corresponds to the note's meta data stored in its front matter. If it is not the case, the
/// note's filename will be renamed.  Can be modified through editing the configuration file.
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
pub const TMPL_VAR_NOTE_BODY: &str = "note_body";

/// HTML template variable containing the automatically generated JavaScript
/// code to be included in the HTML rendition.
/// We could set  
/// `#[cfg(feature = "viewer")]`,
/// but we prefer the same config file structure independent
/// of the enabled features.
pub const TMPL_VAR_NOTE_JS: &str = "note_js";

/// HTML template variable used in the error page containing the error message
/// explaining why this page could not be rendered.
/// We could set  
/// `#[cfg(feature = "viewer")]`,
/// but we prefer the same config file structure independent
/// of the enabled features.
#[allow(dead_code)]
pub const TMPL_VAR_NOTE_ERROR: &str = "note_error";

/// HTML template variable used in the error page containing a verbatim
/// HTML rendition with hyperlinks of the erroneous note file.
/// We could set  
/// `#[cfg(feature = "viewer")]`,
/// but we prefer the same config file structure independent
/// of the enabled features.
#[allow(dead_code)]
pub const TMPL_VAR_NOTE_ERRONEOUS_CONTENT: &str = "note_erroneous_content";

/// HTML template to render regular viewer pages.
/// We could set  
/// `#[cfg(feature = "viewer")]`,
/// but we prefer the same config file structure independent
/// of the enabled features.
pub const VIEWER_RENDITION_TMPL: &str = r#"<!DOCTYPE html>
<html lang="{{ fm_lang | default(value='en') }}">
<head>
<meta charset="UTF-8">
<title>{{ fm_title }}</title>
<style>
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
  <div class="note-body">{{ note_body }}</div>
  <script>{{ note_js }}</script>
</body>
</html>
"#;

/// HTML template to render the viewer-error page.
/// We could set  
/// `#[cfg(feature = "viewer")]`,
/// but we prefer the same config file structure independent
/// of the enabled features.
pub const VIEWER_ERROR_TMPL: &str = r#"<!DOCTYPE html>
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
{{ note_erroneous_content }}
<script>{{ note_js }}</script>
</body>
</html>
"#;

/// HTML template used to render a note into html when the
/// rendition is saved to disk. Similar to `VIEWER_RENDITION_TMPL`
/// but does not inject JavaScript code.
pub const EXPORTER_RENDITION_TMPL: &str = r#"<!DOCTYPE html>
<html lang="{{ fm_lang | default(value='en') }}">
<head>
<meta charset="utf-8">
<title>{{ fm_title }}</title>
<style>
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
  <div class="note-body">{{ note_body }}</div>
</body>
</html>
"#;

lazy_static! {
/// Global variable containing the filename related configuration data.
    pub static ref CFG2: RwLock<Cfg> = RwLock::new(Cfg::default());
}

/// Configuration data, deserialized from the configuration file.
#[derive(Debug, Serialize, Deserialize, Default)]
pub struct Cfg {
    /// Version number of the config file as String -or-
    /// a text message explaining why we could not load the
    /// configuration file.
    pub filename: Filename,
    pub tmpl: Tmpl,
    pub html_tmpl: HtmlTmpl,
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

/// Filename templates and content templates, deserialized from the configuration file.
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Tmpl {
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
pub struct HtmlTmpl {
    // TODO: rename, move to `config2::Rendition.exporter_tmpl`
    pub exporter_tmpl: String,
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

/// Default values for templates.
impl ::std::default::Default for Tmpl {
    fn default() -> Self {
        Tmpl {
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

/// Default values for the exporter feature.
impl ::std::default::Default for HtmlTmpl {
    fn default() -> Self {
        HtmlTmpl {
            exporter_tmpl: EXPORTER_RENDITION_TMPL.to_string(),
        }
    }
}
