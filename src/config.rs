//! Collects `tp-note`'s configuration from a configuration file,
//! the command-line parameters. It also reads the clipboard.

use crate::content::Content;
use crate::filename;
use crate::VERSION;
use anyhow::anyhow;
use atty::{is, Stream};
use clipboard::ClipboardContext;
use clipboard::ClipboardProvider;
use directories::ProjectDirs;
use lazy_static::lazy_static;
use serde::{Deserialize, Serialize};
use std::fs;
use std::io;
use std::io::Read;
use std::path::Path;
use std::path::PathBuf;
use std::pin::Pin;
use structopt::StructOpt;

/// Name of this executable (without the Windows ".exe" extension).
const CURRENT_EXE: &str = "tp-note";

/// Crate `confy` version 0.4 uses this filename by default.
const CONFIG_FILENAME: &str = "tp-note.toml";

/// File extension of `to-note` files.
pub const EXTENSION_DEFAULT: &str = "md";

/// The variables `NOTE_FILE_EXTENSIONS_*` list file extensions that Tp-Note
/// considers as its own note-files.
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

/// See above.
///
/// The present list contains file extensions of
/// RestructuredText encoded Tp-Note files.
pub const NOTE_FILE_EXTENSIONS_RST: &[&str] = &["rst", "rest"];

/// See above.
///
/// The present list contains file extensions of
/// HTML encoded Tp-Note files. For these
/// file types their content is forwarded to the web browser
/// without modification.
pub const NOTE_FILE_EXTENSIONS_HTML: &[&str] = &["htmlnote"];

/// See above.
///
/// The present list contains file extensions of
/// Text encoded Tp-Note files that the viewer shows
/// literally without (almost) any additional rendering.
pub const NOTE_FILE_EXTENSIONS_TXT: &[&str] = &["txtnote", "t2t", "textile", "twiki", "mediawiki"];

/// See above.
///
/// The present list contains file extensions of
/// Tp-Note files for which no viewer is opened
/// (unless Tp-Note is invoked with `--view`).
pub const NOTE_FILE_EXTENSIONS_UNKNOWN: &[&str] = &["adoc", "asciidoc"];

/// Maximum length of a note's filename in bytes. If a filename-template produces
/// a longer string, it will be truncated.
#[cfg(not(test))]
pub const NOTE_FILENAME_LEN_MAX: usize =
    // Most filesystem's limit.
    255
    // Additional separator.
    - COPY_COUNTER_EXTRA_SEPARATOR.len()
    // Additional copy counter.
    - COPY_COUNTER_OPENING_BRACKETS.len() - 2 - COPY_COUNTER_CLOSING_BRACKETS.len()
    // Extra spare bytes, in case the user's copy counter is longer.
    - 6;
#[cfg(test)]
pub const NOTE_FILENAME_LEN_MAX: usize = 10;

/// Default content-template used when the command-line argument <sanit> is a directory. Can be
/// changed through editing the configuration file.
/// The following variables are  defined:
/// `{{ sanit | stem }}`, `{{ file | stem }}`, `{{ file | ext }}`, `{{ extension_default }}` `{{
/// file | tag }}`, `{{ username }}`, `{{ date }}`, `{{ lang }}`, `{{ path }}`.
/// In addition all environment variables can be used, e.g.  `{{ get_env(name=\"LOGNAME\") }}`
/// When placed in YAML-front-matter, the filter `| json_encode` must be appended to each variable.
const TMPL_NEW_CONTENT: &str = "\
---
title:      {{ path | stem | cut | json_encode }}
subtitle:   {{ 'Note' | json_encode }}
author:     {{ username | json_encode }}
date:       {{ now() | date(format='%Y-%m-%d') | json_encode }}
lang:       {{ get_env(name='LANG', default='') | json_encode }}
revision:   {{ '1.0' | json_encode }}
---


";

/// Default filename-template for a new note file on disk. It implements the sync criteria for
/// note-meta data in front-matter and filename.
/// Useful variables in this context are:
/// `{{ title| sanit }}`, `{{ subtitle| sanit }}`, `{{ extension_default }}`,
/// All variables also exist in a `{{ <var>| sanit(alpha) }}` variant: in case its value starts
/// with a number, the string is prepended with `'`.  The first non-numerical variable must be some
/// `{{ <var>| sanit(alpha) }}` variant.
/// Note, as this is filename-template, all variables (except `now` and `extension_default` must be
/// filtered by a `sanit` or `sanit(alpha=true)` filter.
const TMPL_NEW_FILENAME: &str = "\
{{ now() | date(format='%Y%m%d') }}-\
{{ fm_title | sanit(alpha=true) }}{% if fm_subtitle | default(value='') | sanit != '' %}--{% endif %}\
{{ fm_subtitle | default(value='') | sanit  }}{{ extension_default | prepend_dot }}\
";

/// Default template used, when the clipboard or the input stream `stdin` contains a string and one
/// the of these strings contains a valid YAML front matter section.
/// The clipboards body is in `{{ clipboard }}`, the header is in `{{ clipboard_header }}`.  The
/// stdin's body is in `{{ stdin }}`, the header is in `{{ stdin_header }}`.
/// First all variables defined in the clipboard's front matter are registered, the the ones
/// defined in the input stream `stdin`. The latter can overwrite the former.  One of the front
/// matters must define the `title` variable, which is then available in this template as `{{
/// fm_title }}`.
/// When placed in YAML-front-matter, the filter `| json_encode` must be
/// appended to each variable.
const TMPL_COPY_CONTENT: &str = "\
---
title:      {{ fm_title | default(value = path|stem) | cut | json_encode }}
subtitle:   {{ fm_subtitle | default(value = 'Note') | cut | json_encode }}
author:     {{ fm_author | default(value=username) | json_encode }}
date:       {{ fm_date | default(value = now()|date(format='%Y-%m-%d')) | json_encode }}
lang:       {{ fm_lang | default(value = get_env(name='LANG', default='')) | json_encode }}
revision:   {{ fm_revision | default(value = '1.0') | json_encode }}
{% for k, v in fm_all\
 | remove(var='fm_title')\
 | remove(var='fm_subtitle')\
 | remove(var='fm_author')\
 | remove(var='fm_date')\
 | remove(var='fm_lang')\
 | remove(var='fm_revision') %}\
{{ k }}:\t\t{{ v | json_encode }}
{% endfor %}\
---

{{ stdin ~ clipboard }}

";

/// Default filename template used when the stdin or the clipboard contains a string and one of
/// them has a valid YAML header.  Useful variables in this context are:
/// `{{ title|sanit(alpha=true) }}`, `{{ subtitle| sanit }}`, `{{ extension_default }}`, All
/// variables also exist in a `{{ <var>| sanit(alpha) }}` variant: in case its value starts with a
/// number, the string is prepended with `'`.  The first non-numerical variable must be some `{{
/// <var>| sanit(alpha=true) }}` variant.  Note, that in this filename-template, all variables
/// (except `fm_sort_tag`, `fm_file_ext` and `extension_default`) must be filtered by a `sanit` or
/// `sanit(alpha=true)` filter.
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
/// hyper-link in markdown format: [<link-name>](<link-url>), its first part is stored in `{{
/// clipboard | linkname }}`, the second part in `{{ clipboard | linkurl }}`.  The following
/// variables are defined: `{{ dir | stem }}`, `{{ file | stem }}`, `{{ file_ext }}`,
/// `{{ extension_default }}` `{{ path }}`, `{{ file | tag }}`, `{{ username }}`.  In addition all
/// environment variables can be used, e.g.  `{{ get_env(name=\"LOGNAME\") }}`
/// When placed in YAML-front-matter, the filter `| json_encode` must be appended to each variable.
/// Trick: the expression `{% if clipboard != clipboard | heading %}` detects if the clipboard
/// content has more than one line of text.
const TMPL_CLIPBOARD_CONTENT: &str = "\
---
{% if stdin ~ clipboard | linkname !='' %}\
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
revision:   {{ '1.0' | json_encode }}
---

{{ stdin ~ clipboard }}

";

/// Default filename template used when the stdin ~ clipboard contains a string.  Useful variables
/// in this context are: `{{ title| sanit(alpha=true) }}`, `{{ subtitle| sanit }}`, `{{
/// extension_default }}`, All variables also exist in a `{{ <var>| sanit(alpha) }}` variant: in
/// case its value starts with a number, the string is prepended with `'`.  The first non-numerical
/// variable must be some `{{ <var>| sanit(alpha) }}` variant.  Note, that in this
/// filename-template, all variables (except `now` and `extension_default`) must be filtered by a
/// `sanit` or `sanit(alpha=true)` filter.
const TMPL_CLIPBOARD_FILENAME: &str = "\
{{ now() | date(format='%Y%m%d-') }}\
{{ fm_title | sanit(alpha=true) }}\
{% if fm_subtitle | default(value='') | sanit != '' %}--{% endif %}\
{{ fm_subtitle | default(value='') | sanit  }}{{ extension_default | prepend_dot }}\
";

/// Default template used when the command-line <path> parameter points to an existing
/// non-`.md`-file. Can be modified through editing the configuration file.  The following
/// variables are  defined: `{{ file | dirname }}`, `{{ file | stem }}`, `{{ file_ext }}`,
/// `{{ extension_default }}` `{{ file | tag }}`, `{{ username }}`, `{{ lang }}`, `{{ path }}`.  In
/// addition all environment variables can be used, e.g.  `{{ get_env(name=\"LOGNAME\") }}`
/// When placed in YAML-front-matter, the filter `| json_encode` must be appended to each variable.
/// Trick: the expression `{% if stdin ~ clipboard != stdin ~ clipboard | heading %}` detects
/// if the stdin ~ clipboard content has more than one line of text.
const TMPL_ANNOTATE_CONTENT: &str = "\
---
title:      {% filter json_encode %}{{ file | stem }}{{ file | ext | prepend_dot }}{% endfilter %}
{% if stdin ~ clipboard | linkname !='' and stdin ~ clipboard | heading == stdin ~ clipboard %}\
subtitle:   {{ 'URL' | json_encode }}
{% else %}\
subtitle:   {{ 'Note' | json_encode }}
{% endif %}\
author:     {{ username | json_encode }}
date:       {{ now() | date(format='%Y-%m-%d') | json_encode }}
lang:       {{ get_env(name='LANG', default='') | json_encode }}
revision:   {{ '1.0' | json_encode }}
---

[{{ file | tag }}{{ file | stem }}{{ file | ext | prepend_dot }}]\
(<{{ file | tag }}{{ file | stem }}{{ file | ext | prepend_dot }}>)
{% if stdin ~ clipboard != '' %}{% if stdin ~ clipboard != stdin ~ clipboard | heading %}
---
{% endif %}
{{ stdin ~ clipboard }}
{% endif %}
";

/// Filename of a new note, that annotates an existing file on disk given in
/// <path>.
/// Useful variables are:
/// `{{ title | sanit(alpha=true) }}`, `{{ subtitle | sanit }}`, `{{ extension_default }}`.  All
/// variables also exist in a `{{ <var>| sanit(alpha) }}` variant: in case its value starts with a
/// number, the string is prepended with `'`.  The first non-numerical variable must be the `{{
/// <var>| sanit(alpha) }}` variant.
/// Note, that in this filename-template, all variables (expect `file | tag` and
/// `extension_default`) must be filtered by a `sanit` or `sanit(alpha=true)` filter.
const TMPL_ANNOTATE_FILENAME: &str = "\
{{ file | tag }}{{ fm_title | sanit(alpha=true) }}\
{% if fm_subtitle | default(value='') | sanit != '' %}--{% endif %}\
{{ fm_subtitle | default(value='') | sanit }}{{ extension_default | prepend_dot }}\
";

/// Default filename-template to test, if the filename of an existing note file on disk,
/// corresponds to the note's meta data stored in its front matter. If it is not the case, the
/// note's filename will be renamed.  Can be modified through editing the configuration file.
/// Useful variables in this context are:
/// `{{ file | tag }}` `{{ title | sanit }}`, `{{ subtitle | sanit }}`, `{{ ext_default }}`,
/// All variables also exist in a `{{ <var>| sanit(alpha) }}` variant: in case its value starts
/// with a number, the string is prepended with `'`.  `{{ file | tag  }}` must be the first in line
/// here, then followed by a `{{ <var>| sanit(alpha) }}` variable.
/// Note, that in this filename-template, all variables (except `file | tag` and `file | ext`) must
/// be filtered by a `sanit` or `sanit(alpha=true)` filter.
const TMPL_SYNC_FILENAME: &str = "\
{{ fm_sort_tag | default(value = file | tag) }}\
{{ fm_title | default(value='No title') | sanit(alpha=true) }}\
{% if fm_subtitle | default(value='') | sanit != '' %}--{% endif %}\
{{ fm_subtitle | default(value='') | sanit  }}\
{{ fm_file_ext | default(value = file | ext) | prepend_dot }}\
";

/// As all application logic is encoded in Tp-Note's templates, it does not know about field names.
/// Nevertheless it is useful to identify at least one field as _the_ field that identifies a note
/// the most.  When `TMPL_COMPULSORY_FIELD_CONTENT` is not empty, Tp-Note will not synchronize the
/// note's filename and will pop up an error message, unless it finds the field in the note's
/// header.  When `TMPL_COMPULSORY_FIELD_CONTENT` is empty, all files are synchronized without any
/// further field check. Make sure to define a default value with `fm_* | default(value=*)`
/// in case the variable `fm_*` does not exist in the note's front matter.
const TMPL_COMPULSORY_FIELD_CONTENT: &str = "title";

/// Default command-line argument list when launching external editor.
/// The editor list is executed item by item until an editor is found.
/// Can be changed in config file.
#[cfg(all(target_family = "unix", not(target_vendor = "apple")))]
const EDITOR_ARGS: &[&[&str]] = &[
    &["code", "-w", "-n"],
    &["flatpak", "run", "com.visualstudio.code", "-w", "-n"],
    &["atom", "-w"],
    &["flatpak", "run", "com.github.marktext.marktext"],
    &["marktext", "--no-sandbox"],
    &["typora"],
    &["retext"],
    &["geany", "-s", "-i", "-m"],
    &["gedit", "-w"],
    &["mousepad"],
    &["leafpad"],
    &["nvim-qt", "--nofork"],
    &["gvim", "--nofork"],
];
#[cfg(target_family = "windows")]
const EDITOR_ARGS: &[&[&str]] = &[
    &[
        "C:\\Program Files\\Mark Text\\Mark Text.exe",
        "--new-window",
    ],
    &["C:\\Program Files\\Typora\\Typora.exe"],
    &[
        "C:\\Program Files\\Notepad++\\notepad++.exe",
        "-nosession",
        "-multiInst",
    ],
    &["C:\\Windows\\notepad.exe"],
];
// Some info about lauching programs on iOS:
//[dshell.pdf](https://www.stata.com/manuals13/dshell.pdf)
#[cfg(all(target_family = "unix", target_vendor = "apple"))]
const EDITOR_ARGS: &[&[&str]] = &[
    &["/Applications/TextEdit.app/Contents/MacOS/TextEdit"],
    &["/Applications/Mark\\ Text.app/Contents/MacOS/Mark\\ Text"],
];

/// Default command-line argument list when launching an external editor
/// and no graphical environment is available (`DISPLAY=''`).
/// This lists console file editors only.
/// The editor list is executed item by item until an editor is found.
/// Can be changed in config file.
#[cfg(all(target_family = "unix", not(target_vendor = "apple")))]
const EDITOR_CONSOLE_ARGS: &[&[&str]] = &[&["nano"], &["nvim"], &["vim"], &["vi"]];
#[cfg(target_family = "windows")]
const EDITOR_CONSOLE_ARGS: &[&[&str]] = &[&[]];
// Some info about lauching programs on iOS:
// [dshell.pdf](https://www.stata.com/manuals13/dshell.pdf)
#[cfg(all(target_family = "unix", target_vendor = "apple"))]
const EDITOR_CONSOLE_ARGS: &[&[&str]] = &[&["nano"], &["nvim"], &["vim"], &["vi"]];

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

/// Launches a filewatcher, (Markdown)-renderer, html server
/// and a web-browser to view the current note file.
/// To disable this feature, set to false.
/// TODO: Under Windows only Firefox is working at
/// the moment. This is why the switch is off by default.
#[cfg(not(target_family = "windows"))]
const VIEWER_ENABLED: bool = true;
#[cfg(target_family = "windows")]
const VIEWER_ENABLED: bool = false;

/// How often should the file watcher check for changes?
/// Delay in milliseconds.
const VIEWER_NOTIFY_PERIOD: u64 = 1000;

/// Template used to render a note into html.
pub const VIEWER_RENDITION_TMPL: &str = r#"
<!DOCTYPE html>
<html lang="{{ fm_lang | default(value='en') }}">
<head>
<meta charset="utf-8">
<title>{{ fm_title }}</title>
  </head>
  <body>
  <pre class="noteHeader">{{ fm_all_yaml }}</pre>
  <hr>
  <div class="noteBody">{{ noteBody }}</div>
  <script>{{ noteJS }}</script>
</body>
</html>
"#;

/// Template used to render error messages into html.
pub const VIEWER_ERROR_TMPL: &str = r#"
<!DOCTYPE html>
<html lang=\"en\">
<head>
<meta charset=\"utf-8\">
<title>Syntax error</title>
</head>
<body>
<h3>Syntax error</h3>
<p> in note file: <pre>{{ file }}</pre><p>
<hr>
<pre class=\"noteError\">{{ noteError }}<pre>
<script>{{ noteJS }}</script>
</body>
</html>
"#;

#[derive(Debug, PartialEq, StructOpt)]
#[structopt(
    name = "Tp-Note",
    about = "Fast note taking with templates and filename synchronization."
)]
/// `Tp-Note` is a note-taking-tool and a template system, that consistently
/// synchronizes the note's meta-data with its filename. `tp-note` collects
/// various information about its environment and the clipboard and stores them
/// in variables. New notes are created by filling these variables in predefined
/// and customizable `Tera`-templates. In case `<path>` points to an existing
/// `tp-note`-file, the note's meta-data is analysed and, if necessary, its
/// filename is modified. For all other file types, `tp-note` creates a new note
/// that annotates the file `<path>` points to. If `<path>` is a directory (or,
/// when omitted the current working directory), a new note is created in that
/// directory. After creation, `tp-note` launches an external editor of your
/// choice. Although the note's structure follows `pandoc`-conventions, it is not
/// tied to any specific markup language.
pub struct Args {
    /// Batch made: does not launch editor or viewer
    #[structopt(long, short = "b")]
    pub batch: bool,
    /// Loads alternative configuration file
    #[structopt(long, short = "c")]
    pub config: Option<String>,
    /// Debug: shows templates and its variables
    #[structopt(long, short = "d")]
    pub debug: bool,
    /// Launches only the editor, no browser
    #[structopt(long, short = "e")]
    pub edit: bool,
    /// Let web server listen to a specific port
    #[structopt(long, short = "p")]
    pub port: Option<u16>,
    /// Launches only the browser, no editor
    #[structopt(long, short = "v")]
    pub view: bool,
    /// <dir> as new note location or <file> to annotate
    #[structopt(name = "PATH", parse(from_os_str))]
    pub path: Option<PathBuf>,
    /// Prints version and exits
    #[structopt(long, short = "V")]
    pub version: bool,
}

lazy_static! {
/// Structure to hold the parsed command-line arguments.
pub static ref ARGS : Args = Args::from_args();
}

/// Configuration data, deserialized from the configuration-file.
#[derive(Debug, Serialize, Deserialize)]
pub struct Cfg {
    /// Version number of the config file as String -or-
    /// a text message explaining why we could not load the
    /// configuration file.
    pub version: String,
    pub extension_default: String,
    pub note_file_extensions_md: Vec<String>,
    pub note_file_extensions_rst: Vec<String>,
    pub note_file_extensions_html: Vec<String>,
    pub note_file_extensions_txt: Vec<String>,
    pub note_file_extensions_unknown: Vec<String>,
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
    pub clipboard_read_enabled: bool,
    pub clipboard_empty_enabled: bool,
    pub copy_counter_extra_separator: String,
    pub copy_counter_opening_brackets: String,
    pub copy_counter_closing_brackets: String,
    pub viewer_enabled: bool,
    pub viewer_notify_period: u64,
    pub viewer_rendition_tmpl: String,
    pub viewer_error_tmpl: String,
}

/// When no configuration-file is found, defaults are set here from built-in
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
            note_file_extensions_unknown: NOTE_FILE_EXTENSIONS_UNKNOWN
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
            clipboard_read_enabled: CLIPBOARD_READ_ENABLED,
            clipboard_empty_enabled: CLIPBOARD_EMPTY_ENABLED,
            copy_counter_extra_separator: COPY_COUNTER_EXTRA_SEPARATOR.to_string(),
            copy_counter_opening_brackets: COPY_COUNTER_OPENING_BRACKETS.to_string(),
            copy_counter_closing_brackets: COPY_COUNTER_CLOSING_BRACKETS.to_string(),
            viewer_enabled: VIEWER_ENABLED,
            viewer_notify_period: VIEWER_NOTIFY_PERIOD,
            viewer_rendition_tmpl: VIEWER_RENDITION_TMPL.to_string(),
            viewer_error_tmpl: VIEWER_ERROR_TMPL.to_string(),
        }
    }
}

lazy_static! {
    /// Shall we launch the external text editor?
    pub static ref LAUNCH_EDITOR: bool = {
        !ARGS.batch &&
        match (ARGS.edit, ARGS.view, CFG.viewer_enabled) {
            (false, false, false) => true,
            (false, false, true) => true,
            (false, true, false) => false,
            (false, true, true) => false,
            (true, false, false) => true,
            (true, false, true) => true,
            (true, true, false) => true,
            (true, true, true) => true,

        }
    };
}

lazy_static! {
    /// Shall we launch the internal server and the external browser?
    pub static ref LAUNCH_VIEWER: bool = {
        !ARGS.batch && !*RUNS_ON_CONSOLE &&
        match (ARGS.edit, ARGS.view, CFG.viewer_enabled) {
            (false, false, false) => false,
            (false, false, true) => true,
            (false, true, false) => true,
            (false, true, true) => true,
            (true, false, false) => false,
            (true, false, true) => false,
            (true, true, false) => true,
            (true, true, true) => true,
        }
    };
}

lazy_static! {
    /// Do we run on a console?
    pub static ref RUNS_ON_CONSOLE: bool = {
        // On Linux popup window only if DISPLAY is set.
        #[cfg(target_family = "unix")]
        let display = std::env::var("DISPLAY")
            // Map error to `None`.
            .ok()
            // A pattern mapping `Some("")` to `None`.
            .and_then(|s: String| if s.is_empty() { None } else { Some(s) });

        // In non-Linux there is always "Some" display.
        #[cfg(not(target_family = "unix"))]
        let display = Some(String::new());

        display.is_none()
    };
}

#[cfg(not(test))]
lazy_static! {
    /// Reads and parses the configuration file "tp-note.toml". An alternative
    /// filename (optionally with absolute path) can be given on the command line
    /// with "--config".
    pub static ref CFG: Cfg = confy::load::<Cfg>(PathBuf::from(
        if let Some(c) = &ARGS.config {
            c
        } else {
            CURRENT_EXE
        })
        // strip extension, ".toml" is added by `confy.load()`
        .with_extension("")
        .to_str()
        .unwrap_or_default()
        ).unwrap_or_else(|e|{
            // We are here, because we could not load the config file.
            let mut c = Cfg::default();
            // Instead of a version string, we store the reason why.
            let mut e = e.to_string();
            // This is a marker string that will cause a version parse error on purpose.
            e.push('!');
            c.version = e;
            c
        });
}

#[cfg(test)]
lazy_static! {
    pub static ref CFG: Cfg = Cfg::default();
}

lazy_static! {
/// This is where the `confy` crate stores the configuration file.
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

pub fn backup_config_file() -> Result<PathBuf, anyhow::Error> {
    if let Some(ref config_path) = *CONFIG_PATH {
        if config_path.exists() {
            let config_path_bak = filename::find_unused((&config_path).to_path_buf())?;

            fs::rename(&config_path.as_path(), &config_path_bak)?;

            Ok(config_path_bak)
        } else {
            Err(anyhow!("no file to move"))
        }
    } else {
        Err(anyhow!("no path to configuration file found"))
    }
}

lazy_static! {
    /// Reads the input stream stdin if there is any.
    pub static ref STDIN: Pin<Box<Content<'static>>> = {
        let mut buffer = String::new();

        // Read stdin().
        if !is(Stream::Stdin) {
            let stdin = io::stdin();
            let mut handle = stdin.lock();
            let _ = handle.read_to_string(&mut buffer);
        }

        // `trim_end()` content without new allocation.
        buffer.truncate(buffer.trim_end().len());

        Content::new(buffer, false)
    };
}

lazy_static! {
    /// Reads the clipboard, if there is any and empties it.
    pub static ref CLIPBOARD: Pin<Box<Content<'static>>> = {
        let mut buffer = String::new();

        // Concatenate clipboard content.
        if CFG.clipboard_read_enabled && !*RUNS_ON_CONSOLE && !ARGS.batch {
            let ctx: Option<ClipboardContext> = ClipboardProvider::new().ok();
            if ctx.is_some() {
                let ctx = &mut ctx.unwrap(); // This is ok since `is_some()`
                let s = ctx.get_contents().ok();
                buffer.push_str(&s.unwrap_or_default());
            }
        };

        // `trim_end()` content without new allocation.
        buffer.truncate(buffer.trim_end().len());

        Content::new(buffer, false)
    };
}

#[derive(Debug, PartialEq, Default)]
/// Represents a hyperlink.
pub struct Hyperlink {
    pub name: String,
    pub url: String,
}

impl Hyperlink {
    /// Parse a markdown formatted hyperlink and stores the result in `Self`.
    pub fn new(input: &str) -> Result<Hyperlink, anyhow::Error> {
        // parse input_linkname
        let name_start = input
            .find('[')
            .ok_or_else(|| anyhow!(format!("no `[` in \"{}\"", input)))?
            + 1;

        let mut bracket_counter = 1;
        let name_end = input[name_start..]
            .find(|c: char| {
                if c == '[' {
                    bracket_counter += 1;
                } else if c == ']' {
                    bracket_counter -= 1;
                };
                bracket_counter == 0
            })
            .ok_or_else(|| anyhow!(format!("no closing`]` in \"{}\"", input)))?
            + name_start;

        // parse input_linkurl
        if input[name_end + 1..].chars().next().unwrap_or('x') != '(' {
            return Err(anyhow!(format!("no `](` in \"{}\"", input)));
        };
        let url_start = name_end + 2;
        let mut bracket_counter = 1;
        let url_end = input[url_start..]
            .find(|c: char| {
                if c == '(' {
                    bracket_counter += 1;
                } else if c == ')' {
                    bracket_counter -= 1;
                };
                bracket_counter == 0
            })
            .ok_or_else(|| anyhow!(format!("no closing `)` in \"{}\"", input)))?
            + url_start;

        Ok(Hyperlink {
            name: input[name_start..name_end].to_string(),
            url: input[url_start..url_end].to_string(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::Hyperlink;

    #[test]
    fn test_parse_hyperlink() {
        // Regular link
        let input = "xxx[Homepage](https://blog.getreu.net)";
        let expected_output = Hyperlink {
            name: "Homepage".to_string(),
            url: "https://blog.getreu.net".to_string(),
        };
        let output = Hyperlink::new(input);
        assert_eq!(expected_output, output.unwrap());

        // URL with ()
        let input = "xxx[Homepage](https://blog.getreu.net/(main))";
        let expected_output = Hyperlink {
            name: "Homepage".to_string(),
            url: "https://blog.getreu.net/(main)".to_string(),
        };
        let output = Hyperlink::new(input);
        assert_eq!(expected_output, output.unwrap());

        //
        // link with () in name
        let input = "[Homepage (my first)](https://getreu.net)";
        let expected_output = Hyperlink {
            name: "Homepage (my first)".to_string(),
            url: "https://getreu.net".to_string(),
        };
        let output = Hyperlink::new(input);
        assert_eq!(expected_output, output.unwrap());

        //
        // link with [] in name
        let input = "[Homepage [my first]](https://getreu.net)";
        let expected_output = Hyperlink {
            name: "Homepage [my first]".to_string(),
            url: "https://getreu.net".to_string(),
        };
        let output = Hyperlink::new(input);
        assert_eq!(expected_output, output.unwrap());

        //
        // link with [ in name
        let input = "[Homepage [my first](https://getreu.net)";
        let output = Hyperlink::new(input);
        assert!(output.is_err());

        //
        // link with only []
        let input = "[Homepage (my first)]";
        let output = Hyperlink::new(input);
        assert!(output.is_err());
    }
}
