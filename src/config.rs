//! Set configuration defaults, reads and writes _Tp-Note_'s configuration file
//! and exposes the configuration as `static` variable.

use crate::error::FileError;
use crate::filename;
use crate::settings::ARGS;
use crate::VERSION;
#[cfg(feature = "read-clipboard")]
#[cfg(feature = "read-clipboard")]
use directories::ProjectDirs;
use lazy_static::lazy_static;
use log::LevelFilter;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
#[cfg(not(test))]
use std::fs::File;
#[cfg(not(test))]
use std::io::Write;
use std::path::Path;
use std::path::PathBuf;
use std::sync::RwLock;

/// Name of this executable (without the Windows ".exe" extension).
const CURRENT_EXE: &str = "tp-note";

/// Tp-Note's configuration file filename.
const CONFIG_FILENAME: &str = "tp-note.toml";

/// Default value for command line option `--debug`.  Determines the maximum
/// debug level events must have, to be logged.  If the command line option
/// `--debug` is present, its value will be used instead.
const DEBUG_ARG_DEFAULT: LevelFilter = LevelFilter::Error;

/// Default value for command line flag `--edit` to disable file watcher,
/// (Markdown)-renderer, html server and a web browser launcher set to `true`.
const EDITOR_ARG_DEFAULT: bool = false;

/// Default value for command line flag `--popup` If the command line flag
/// `--popup` or `POPUP` is `true`, all log events will also trigger the
/// appearance of a popup alert window.  Note, that error level debug events
/// will always pop up, regardless of `--popup` and `POPUP` (unless
/// `--debug=off`).
const POPUP_ARG_DEFAULT: bool = true;

/// Default value for command line flag `--no-filename-sync` to disable
/// the title to filename synchronisation mechanism permanently.
/// If set to `true`, the corresponding command line flag is ignored.
const NO_FILENAME_SYNC_ARG_DEFAULT: bool = false;

/// _Tp-Note_ opens all `.md` files with an external editor. It recognizes its
/// own files, by the file extension `.md`, by a valid YAML header and the
/// presence of a "title" variable*). When set to `false`, popup alert windows
/// inform the user about missing headers. `true` suppresses the popup alert
/// windows and the text editor starts without further notification.
/// See also `VIEWER_MISSING_HEADER_DISABLES`.
///
/// *) all string literals given in this example are configurable.
const SILENTLY_IGNORE_MISSING_HEADER: bool = true;

/// File extension of `to-note` files.
pub const EXTENSION_DEFAULT: &str = "md";

/// The variables `NOTE_FILE_EXTENSIONS_*` list file extensions that Tp-Note
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
pub const NOTE_FILE_EXTENSIONS_MD: &[&str] = &["md", "markdown", "markdn", "mdown", "mdtxt"];

/// The present list contains file extensions of
/// RestructuredText encoded Tp-Note files.
///
/// See also `NOTE_FILE_EXTENSION_MD`.
pub const NOTE_FILE_EXTENSIONS_RST: &[&str] = &["rst", "rest"];

/// The present list contains file extensions of
/// HTML encoded Tp-Note files. For these
/// file types their content is forwarded to the web browser
/// without modification.
///
/// See also `NOTE_FILE_EXTENSION_MD`.
pub const NOTE_FILE_EXTENSIONS_HTML: &[&str] = &["htmlnote"];

/// The present list contains file extensions of
/// Text encoded Tp-Note files that the viewer shows
/// literally without (almost) any additional rendering.
/// Only hyperlinks in _Markdown_, _reStructuredText_, _Asciidoc_ and _HTML_ are
/// rendered, thus clickable.
///
/// See also `NOTE_FILE_EXTENSION_MD`.
pub const NOTE_FILE_EXTENSIONS_TXT: &[&str] = &["txtnote", "adoc", "asciidoc"];

/// The present list contains file extensions of
/// Tp-Note files for which no viewer is opened
/// (unless Tp-Note is invoked with `--view`).
///
/// See also `NOTE_FILE_EXTENSION_MD`.
pub const NOTE_FILE_EXTENSIONS_NO_VIEWER: &[&str] = &["t2t", "textile", "twiki", "mediawiki"];

/// Maximum length of a note's filename in bytes. If a filename template produces
/// a longer string, it will be truncated.
#[cfg(not(test))]
pub const NOTE_FILENAME_LEN_MAX: usize =
    // Most file system's limit.
    255
    // Additional separator.
    - COPY_COUNTER_EXTRA_SEPARATOR.len()
    // Additional copy counter.
    - COPY_COUNTER_OPENING_BRACKETS.len() - 2 - COPY_COUNTER_CLOSING_BRACKETS.len()
    // Extra spare bytes, in case the user's copy counter is longer.
    - 6;
#[cfg(test)]
pub const NOTE_FILENAME_LEN_MAX: usize = 10;

/// Default content template used when the command line argument <sanit> is a directory. Can be
/// changed through editing the configuration file.
/// The following variables are  defined:
/// `{{ sanit | stem }}`, `{{ path | stem }}`, `{{ path | ext }}`, `{{ extension_default }}` `{{
/// file | tag }}`, `{{ username }}`, `{{ date }}`, `{{ lang }}`, `{{ dir_path }}`.
/// In addition all environment variables can be used, e.g.  `{{ get_env(name=\"LOGNAME\") }}`
/// When placed in YAML front matter, the filter `| json_encode` must be appended to each variable.
const TMPL_NEW_CONTENT: &str = "\
---
title:      {{ dir_path | trim_tag | cut | json_encode }}
subtitle:   {{ 'Note' | json_encode }}
author:     {{ username | json_encode }}
date:       {{ now() | date(format='%Y-%m-%d') | json_encode }}
lang:       {{ get_env(name='LANG', default='') | json_encode }}
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
/// filtered by a `sanit` or `sanit(alpha=true)` filter.
const TMPL_NEW_FILENAME: &str = "\
{{ now() | date(format='%Y%m%d-') }}\
{{ fm_title | sanit(alpha=true) }}{% if fm_subtitle | default(value='') | sanit != '' %}--{% endif %}\
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
const TMPL_COPY_CONTENT: &str = "\
---
title:      {{ fm_title | default(value = path|trim_tag) | cut | json_encode }}
subtitle:   {{ fm_subtitle | default(value = 'Note') | cut | json_encode }}
author:     {{ fm_author | default(value=username) | json_encode }}
date:       {{ fm_date | default(value = now()|date(format='%Y-%m-%d')) | json_encode }}
lang:       {{ fm_lang | default(value = get_env(name='LANG', default='')) | json_encode }}
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
const TMPL_COPY_FILENAME: &str = "\
{{ fm_sort_tag | default(value = now() | date(format='%Y%m%d-')) }}\
{{ fm_title | sanit(alpha=true) }}\
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
/// Trick: the expression `{% if clipboard != clipboard | heading %}` detects if the clipboard
/// content has more than one line of text.
const TMPL_CLIPBOARD_CONTENT: &str = "\
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
{% if stdin ~ clipboard | linkname !='' and stdin ~ clipboard | heading == stdin ~ clipboard %}\
subtitle:   {{ 'URL' | json_encode }}
{% else %}\
subtitle:   {{ 'Note' | json_encode }}
{% endif %}\
author:     {{ username | json_encode }}
date:       {{ now() | date(format='%Y-%m-%d') | json_encode }}
lang:       {{ get_env(name='LANG', default='') | json_encode }}
---

{{ stdin ~ clipboard }}

";

/// Default filename template used when the stdin ~ clipboard contains a string.
const TMPL_CLIPBOARD_FILENAME: &str = "\
{{ now() | date(format='%Y%m%d-') }}\
{{ fm_title | sanit(alpha=true) }}\
{% if fm_subtitle | default(value='') | sanit != '' %}--{% endif %}\
{{ fm_subtitle | default(value='') | sanit  }}{{ extension_default | prepend_dot }}\
";

/// Default template used when the command line <path> parameter points to an existing
/// non-`.md`-file. Can be modified through editing the configuration file.
const TMPL_ANNOTATE_CONTENT: &str = "\
---
title:      {% filter json_encode %}{{ path | stem }}{{ path | copy_counter }}\
{{ path | ext | prepend_dot }}{% endfilter %}
{% if stdin ~ clipboard | linkname !='' and stdin ~ clipboard | heading == stdin ~ clipboard %}\
subtitle:   {{ 'URL' | json_encode }}
{% else %}\
subtitle:   {{ 'Note' | json_encode }}
{% endif %}\
author:     {{ username | json_encode }}
date:       {{ now() | date(format='%Y-%m-%d') | json_encode }}
lang:       {{ get_env(name='LANG', default='') | json_encode }}
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
const TMPL_ANNOTATE_FILENAME: &str = "\
{{ path | tag }}{{ fm_title | sanit(alpha=true) }}\
{% if fm_subtitle | default(value='') | sanit != '' %}--{% endif %}\
{{ fm_subtitle | default(value='') | sanit }}{{ extension_default | prepend_dot }}\
";

/// Default filename template to test, if the filename of an existing note file on disk,
/// corresponds to the note's meta data stored in its front matter. If it is not the case, the
/// note's filename will be renamed.  Can be modified through editing the configuration file.
const TMPL_SYNC_FILENAME: &str = "\
{{ fm_sort_tag | default(value = path | tag) }}\
{{ fm_title | default(value='No title') | sanit(alpha=true) }}\
{% if fm_subtitle | default(value='') | sanit != '' %}--{% endif %}\
{{ fm_subtitle | default(value='') | sanit  }}\
{{ fm_file_ext | default(value = path | ext) | prepend_dot }}\
";

/// As all application logic is encoded in Tp-Note's templates, it does not know about field names.
/// Nevertheless it is useful to identify at least one field as _the_ field that identifies a note
/// the most.  When `TMPL_COMPULSORY_FIELD_CONTENT` is not empty, Tp-Note will not synchronize the
/// note's filename and will pop up an error message, unless it finds the field in the note's
/// header.  When `TMPL_COMPULSORY_FIELD_CONTENT` is empty, all files are synchronized without any
/// further field check. Make sure to define a default value with `fm_* | default(value=*)`
/// in case the variable `fm_*` does not exist in the note's front matter.
const TMPL_COMPULSORY_FIELD_CONTENT: &str = "title";

/// Default command line argument list when launching external editor.
/// The editor list is executed item by item until an editor is found.
/// Can be changed in config file.
#[cfg(all(target_family = "unix", not(target_vendor = "apple")))]
const EDITOR_ARGS: &[&[&str]] = &[
    &["code", "-w", "-n"],
    &["flatpak", "run", "com.visualstudio.code", "-w", "-n"],
    &["atom", "-w"],
    &["marktext", "--no-sandbox", "--new-window"],
    &[
        "flatpak",
        "run",
        "com.github.marktext.marktext",
        "--new-window",
    ],
    &["typora"],
    &["retext"],
    &["geany", "-s", "-i", "-m"],
    &["gedit", "-w"],
    &["mousepad", "--disable-server"],
    &["leafpad"],
    &["nvim-qt", "--nofork"],
    &["gvim", "--nofork"],
];
#[cfg(target_family = "windows")]
const EDITOR_ARGS: &[&[&str]] = &[
    &["C:\\Program Files\\Typora\\Typora.exe"],
    &[
        "C:\\Program Files\\Mark Text\\Mark Text.exe",
        "--new-window",
    ],
    &[
        "C:\\Program Files\\Notepad++\\notepad++.exe",
        "-nosession",
        "-multiInst",
    ],
    &["C:\\Windows\\notepad.exe"],
];
// Some info about launching programs on iOS:
//[dshell.pdf](https://www.stata.com/manuals13/dshell.pdf)
#[cfg(all(target_family = "unix", target_vendor = "apple"))]
const EDITOR_ARGS: &[&[&str]] = &[
    &["code", "-w", "-n"],
    &["atom", "-w"],
    &["marktext", "--no-sandbox"],
    &["typora"],
    &["gvim", "--nofork"],
    &["mate"],
    &["open", "-a", "TextEdit"],
    &["open", "-a", "TextMate"],
    &["open"],
];

/// Default command line argument list when launching an external editor
/// and no graphical environment is available (`DISPLAY=''`).
/// This lists console file editors only.
/// The editor list is executed item by item until an editor is found.
/// Can be changed in config file.
#[cfg(all(target_family = "unix", not(target_vendor = "apple")))]
const EDITOR_CONSOLE_ARGS: &[&[&str]] = &[&["nvim"], &["nano"], &["vim"], &["emacs"], &["vi"]];
#[cfg(target_family = "windows")]
const EDITOR_CONSOLE_ARGS: &[&[&str]] = &[&[]];
// Some info about launching programs on iOS:
// [dshell.pdf](https://www.stata.com/manuals13/dshell.pdf)
#[cfg(all(target_family = "unix", target_vendor = "apple"))]
const EDITOR_CONSOLE_ARGS: &[&[&str]] = &[
    &["nvim"],
    &["nano"],
    &["pico"],
    &["vim"],
    &["emacs"],
    &["vi"],
];

/// Default command line argument list when launching the web browser.
/// The list is executed item by item until an installed web browser is found.
/// Can be changed in config file.
#[cfg(all(target_family = "unix", not(target_vendor = "apple")))]
const BROWSER_ARGS: &[&[&str]] = &[
    &[
        "flatpak",
        "run",
        "org.mozilla.firefox",
        "--new-window",
        "--private-window",
    ],
    &["firefox", "--new-window", "--private-window"],
    &["firefox-esr", "--new-window", "--private-window"],
    &[
        "flatpak",
        "run",
        "com.github.Eloston.UngoogledChromium",
        "--new-window",
        "--incognito",
    ],
    &[
        "flatpak",
        "run",
        "org.chromium.Chromium",
        "--new-window",
        "--incognito",
    ],
    &["chromium-browser", "--new-window", "--incognito"],
    &["chrome", "--new-window", "--incognito"],
];
#[cfg(target_family = "windows")]
const BROWSER_ARGS: &[&[&str]] = &[
    &[
        "C:\\Program Files\\Mozilla Firefox\\firefox.exe",
        "--new-window",
        "--private-window",
    ],
    &[
        "C:\\Program Files\\Google\\Chrome\\Application\\chrome",
        "--new-window",
        "--incognito",
    ],
    &[
        "C:\\Program Files (x86)\\Microsoft\\Edge\\Application\\msedge.exe",
        "--inprivate",
    ],
];
// Some info about launching programs on iOS:
//[dshell.pdf](https://www.stata.com/manuals13/dshell.pdf)
#[cfg(all(target_family = "unix", target_vendor = "apple"))]
const BROWSER_ARGS: &[&[&str]] = &[];

/// By default clipboard support is enabled, can be disabled
/// in config file. A false value here will set ENABLE_EMPTY_CLIPBOARD to
/// false.
const CLIPBOARD_READ_ENABLED: bool = true;

/// Should the clipboard be emptied when tp-note closes?
/// Default value.
const CLIPBOARD_EMPTY_ENABLED: bool = true;

/// If the stem of a filename ends with a pattern, that is similar
/// to a copy counter, add this extra separator. Must be `-`, `_`
/// or any combination of both. Shorter looks better.
const COPY_COUNTER_EXTRA_SEPARATOR: &str = "-";

/// Tp-Note may add a counter at the end of the filename when
/// it can not save a file because the name is taken already.
/// This is the opening bracket search pattern. Some examples:
/// `"-"`, "'_'"", `"_-"`,`"-_"`, `"("`
/// Can be empty.
const COPY_COUNTER_OPENING_BRACKETS: &str = "(";

/// Tp-Note may add a counter at the end of the filename when
/// it can not save a file because the name is taken already.
/// This is the closing bracket search pattern. Some examples:
/// `"-"`, "'_'"", `"_-"`,`"-_"`, `"("`
/// Can be empty.
const COPY_COUNTER_CLOSING_BRACKETS: &str = ")";

/// When a filename is taken already, Tp-Note adds a copy
/// counter number in the range of `0..COPY_COUNTER_MAX`
/// at the end.
pub const COPY_COUNTER_MAX: usize = 400;

/// When set to true, the viewer feature is automatically disabled when
/// _Tp-Note_ encounters an `.md` file without header.  Experienced users can
/// set this to `true`. See also `SILENTLY_IGNORE_MISSING_HEADER`.
const VIEWER_MISSING_HEADER_DISABLES: bool = false;

/// How often should the file watcher check for changes?
/// Delay in milliseconds.
const VIEWER_NOTIFY_PERIOD: u64 = 1000;

/// The maximum number of TCP connections the HTTP server can handle at the same
/// time. In general, the serving and live update of the HTML rendition of the
/// note file, requires normally 3 TCP connections: 1 old event channel (that is
/// still open from the previous update), 1 TCP connection to serve the HTML,
/// the local images (and referenced documents), and 1 new event channel.  In
/// practise, stale connection are not always closed immediately. Hence 4 open
/// connections are not uncommon.
const VIEWER_TCP_CONNECTIONS_MAX: usize = 16;

/// Served file types with corresponding mime types.  First entry per line is
/// the file extension in lowercase, the second the corresponding mime type.
/// Embedded files with types other than those listed here are silently ignored.
/// Note, that image files must be located in the same or in the note's parent
/// directory.
const VIEWER_SERVED_MIME_TYPES: &[&[&str]] = &[
    &["apng", "image/apng"],
    &["avif", "image/avif"],
    &["bmp", "image/bmp"],
    &["gif", "image/gif"],
    &["html", "text/html"],
    &["htm", "text/html"],
    &["ico", "image/vnd.microsoft.icon"],
    &["jpeg", "image/jpeg"],
    &["jpg", "image/jpeg"],
    &["pdf", "application/pdf"],
    &["png", "image/png"],
    &["svg", "image/svg+xml"],
    &["tiff", "image/tiff"],
    &["tif", "image/tiff"],
    &["webp", "image/webp"],
    &["mp3", "audio/mp3"],
    &["ogg", "audio/ogg"],
    &["oga", "audio/ogg"],
    &["weba", "audio/webm"],
    &["flac", "audio/flac"],
    &["wav", "audio/wav"],
    &["opus", "audio/opus"],
    &["mp4", "video/mp4"],
    &["ogv", "video/ogg"],
    &["webm", "video/webm"],
    &["ogx", "application/ogg"],
];

/// Template used by the viewer to render a note into html.
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

/// Template used by the viewer to render error messages into html.
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

/// Template used to render a note into html when the
/// rendition is saved to disk
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

/// Configuration data, deserialized from the configuration file.
#[derive(Debug, Serialize, Deserialize)]
pub struct Cfg {
    /// Version number of the config file as String -or-
    /// a text message explaining why we could not load the
    /// configuration file.
    pub version: String,
    pub debug_arg_default: LevelFilter,
    pub edit_arg_default: bool,
    pub no_filename_sync_arg_default: bool,
    pub popup_arg_default: bool,
    pub silently_ignore_missing_header: bool,
    pub extension_default: String,
    pub note_file_extensions_md: Vec<String>,
    pub note_file_extensions_rst: Vec<String>,
    pub note_file_extensions_html: Vec<String>,
    pub note_file_extensions_txt: Vec<String>,
    pub note_file_extensions_no_viewer: Vec<String>,
    pub tmpl_new_content: String,
    pub tmpl_new_filename: String,
    pub tmpl_copy_content: String,
    pub tmpl_copy_filename: String,
    pub tmpl_clipboard_content: String,
    pub tmpl_clipboard_filename: String,
    pub tmpl_annotate_content: String,
    pub tmpl_annotate_filename: String,
    pub tmpl_sync_filename: String,
    pub tmpl_compulsory_field_content: String,
    pub editor_args: Vec<Vec<String>>,
    pub editor_console_args: Vec<Vec<String>>,
    pub browser_args: Vec<Vec<String>>,
    pub clipboard_read_enabled: bool,
    pub clipboard_empty_enabled: bool,
    pub copy_counter_extra_separator: String,
    pub copy_counter_opening_brackets: String,
    pub copy_counter_closing_brackets: String,
    pub viewer_missing_header_disables: bool,
    pub viewer_notify_period: u64,
    pub viewer_tcp_connections_max: usize,
    pub viewer_served_mime_types: Vec<Vec<String>>,
    pub viewer_rendition_tmpl: String,
    pub viewer_error_tmpl: String,
    pub exporter_rendition_tmpl: String,
}

/// When no configuration file is found, defaults are set here from built-in
/// constants. These defaults are then serialized into a newly created
/// configuration file on disk.
impl ::std::default::Default for Cfg {
    fn default() -> Self {
        let version = match VERSION {
            Some(v) => v.to_string(),
            None => "".to_string(),
        };

        Cfg {
            version,
            debug_arg_default: DEBUG_ARG_DEFAULT,
            edit_arg_default: EDITOR_ARG_DEFAULT,
            popup_arg_default: POPUP_ARG_DEFAULT,
            no_filename_sync_arg_default: NO_FILENAME_SYNC_ARG_DEFAULT,
            silently_ignore_missing_header: SILENTLY_IGNORE_MISSING_HEADER,
            extension_default: EXTENSION_DEFAULT.to_string(),
            note_file_extensions_md: NOTE_FILE_EXTENSIONS_MD
                .iter()
                .map(|a| (*a).to_string())
                .collect(),
            note_file_extensions_rst: NOTE_FILE_EXTENSIONS_RST
                .iter()
                .map(|a| (*a).to_string())
                .collect(),
            note_file_extensions_html: NOTE_FILE_EXTENSIONS_HTML
                .iter()
                .map(|a| (*a).to_string())
                .collect(),
            note_file_extensions_txt: NOTE_FILE_EXTENSIONS_TXT
                .iter()
                .map(|a| (*a).to_string())
                .collect(),
            note_file_extensions_no_viewer: NOTE_FILE_EXTENSIONS_NO_VIEWER
                .iter()
                .map(|a| (*a).to_string())
                .collect(),
            tmpl_new_content: TMPL_NEW_CONTENT.to_string(),
            tmpl_new_filename: TMPL_NEW_FILENAME.to_string(),
            tmpl_copy_content: TMPL_COPY_CONTENT.to_string(),
            tmpl_copy_filename: TMPL_COPY_FILENAME.to_string(),
            tmpl_clipboard_content: TMPL_CLIPBOARD_CONTENT.to_string(),
            tmpl_clipboard_filename: TMPL_CLIPBOARD_FILENAME.to_string(),
            tmpl_annotate_content: TMPL_ANNOTATE_CONTENT.to_string(),
            tmpl_annotate_filename: TMPL_ANNOTATE_FILENAME.to_string(),
            tmpl_sync_filename: TMPL_SYNC_FILENAME.to_string(),
            tmpl_compulsory_field_content: TMPL_COMPULSORY_FIELD_CONTENT.to_string(),
            editor_args: EDITOR_ARGS
                .iter()
                .map(|i| i.iter().map(|a| (*a).to_string()).collect())
                .collect(),
            editor_console_args: EDITOR_CONSOLE_ARGS
                .iter()
                .map(|i| i.iter().map(|a| (*a).to_string()).collect())
                .collect(),
            browser_args: BROWSER_ARGS
                .iter()
                .map(|i| i.iter().map(|a| (*a).to_string()).collect())
                .collect(),
            clipboard_read_enabled: CLIPBOARD_READ_ENABLED,
            clipboard_empty_enabled: CLIPBOARD_EMPTY_ENABLED,
            copy_counter_extra_separator: COPY_COUNTER_EXTRA_SEPARATOR.to_string(),
            copy_counter_opening_brackets: COPY_COUNTER_OPENING_BRACKETS.to_string(),
            copy_counter_closing_brackets: COPY_COUNTER_CLOSING_BRACKETS.to_string(),
            viewer_missing_header_disables: VIEWER_MISSING_HEADER_DISABLES,
            viewer_notify_period: VIEWER_NOTIFY_PERIOD,
            viewer_tcp_connections_max: VIEWER_TCP_CONNECTIONS_MAX,
            viewer_served_mime_types: VIEWER_SERVED_MIME_TYPES
                .iter()
                .map(|i| i.iter().map(|a| (*a).to_string()).collect())
                .collect(),
            viewer_rendition_tmpl: VIEWER_RENDITION_TMPL.to_string(),
            viewer_error_tmpl: VIEWER_ERROR_TMPL.to_string(),
            exporter_rendition_tmpl: EXPORTER_RENDITION_TMPL.to_string(),
        }
    }
}

lazy_static! {
    /// Store the extension as key and mime type as value in HashMap.
    pub static ref VIEWER_SERVED_MIME_TYPES_HMAP: HashMap<&'static str, &'static str> = {
        let mut hm = HashMap::new();
        for l in &CFG.viewer_served_mime_types {
            if l.len() >= 2
            {
                hm.insert(l[0].as_str(), l[1].as_str());
            };
        };
        hm
    };
}

lazy_static! {
    /// Variable indicating with `Err` if the loading of the configuration file went wrong.
    pub static ref CFG_FILE_LOADING: RwLock<Result<(), FileError>> = RwLock::new(Ok(()));
}

/// Parse the configuration file if it exists. Otherwise write one with default values.
#[cfg(not(test))]
#[inline]
fn config_load_path(config_path: &Path) -> Result<Cfg, FileError> {
    if config_path.exists() {
        let config: Cfg = toml::from_str(&fs::read_to_string(config_path)?)?;
        Ok(config)
    } else {
        let mut buffer = File::create(config_path)?;
        buffer.write_all(toml::to_string_pretty(&Cfg::default())?.as_bytes())?;
        Ok(Cfg::default())
    }
}

#[cfg(test)]
#[inline]
fn config_load_path(_config_path: &Path) -> Result<Cfg, FileError> {
    Ok(Cfg::default())
}

lazy_static! {
    /// Reads and parses the configuration file "tp-note.toml". An alternative
    /// filename (optionally with absolute path) can be given on the command line
    /// with "--config".
    pub static ref CFG: Cfg = {
        let config_path = if let Some(c) = &ARGS.config {
            Path::new(c)
        } else {
            match &*CONFIG_PATH {
                Some(p) => p.as_path(),
                None => {
                    // Remember that something went wrong.
                    let mut cfg_file_loading = CFG_FILE_LOADING.write().unwrap();
                    *cfg_file_loading = Err(FileError::PathToConfigFileNotFound);
                    return Cfg::default();
                },
            }
        };

        config_load_path(&config_path)
            .unwrap_or_else(|e|{
                // Remember that something went wrong.
                let mut cfg_file_loading = CFG_FILE_LOADING.write().unwrap();
                *cfg_file_loading = Err(e.into());

                // As we could not load the config file, we will use the default
                // configuration.
                Cfg::default()
            })
        };
}

lazy_static! {
/// This is where the Tp-Note stores its configuration file.
    pub static ref CONFIG_PATH : Option<PathBuf> = {
        if let Some(c) = &ARGS.config {
            Some(PathBuf::from(c))
        } else {
            let config = ProjectDirs::from("rs", "", CURRENT_EXE)?;

            let mut config = PathBuf::from(config.config_dir());
            config.push(Path::new(CONFIG_FILENAME));
            Some(config)
        }
    };
}

pub fn backup_config_file() -> Result<PathBuf, FileError> {
    if let Some(ref config_path) = *CONFIG_PATH {
        if config_path.exists() {
            let config_path_bak = filename::find_unused((&config_path).to_path_buf())?;

            fs::rename(&config_path.as_path(), &config_path_bak)?;

            Ok(config_path_bak)
        } else {
            Err(FileError::ConfigFileNotFound)
        }
    } else {
        Err(FileError::PathToConfigFileNotFound)
    }
}
