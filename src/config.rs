//! Collects `tp-note`'s configuration from a configuration file,
//! the command-line parameters. It also reads the clipboard.

extern crate atty;
extern crate clipboard;
extern crate directories;
use crate::error::AlertDialog;
use crate::VERSION;
use anyhow::anyhow;
use atty::{is, Stream};
use clipboard::ClipboardContext;
use clipboard::ClipboardProvider;
use directories::ProjectDirs;
use lazy_static::lazy_static;
use serde::{Deserialize, Serialize};
use std::io;
use std::io::Read;
use std::path::Path;
use std::path::PathBuf;
use std::process;
use structopt::StructOpt;

/// Name of this executable (without the Windows ".exe" extension).
const CURRENT_EXE: &str = "tp-note";

/// Crate `confy` version 0.4 uses this filename by default.
const CONFIG_FILENAME: &str = "tp-note.toml";

/// File extension of `to-note` files.
pub const EXTENSION_DEFAULT: &str = "md";

/// List of file extensions Tp-Note recognizes as note-files and opens to read their YAML header.
/// Files with other file extensions will not be opened by Tp-Note. Instead, a new note is created
/// with the TMPL_ANNOTATE_CONTENT and TMPL_ANNOTATE_FILENAME templates. It is possible to add
/// file extensions of other markup languages than Markdown here, as long as these files come with
/// a valid YAML meta-data header.
pub const NOTE_FILE_EXTENSIONS: &[&str] =
    &[EXTENSION_DEFAULT, "markdown", "markdn", "mdown", "mdtxt"];

/// Maximum length of a note's filename in bytes. If a filename-template produces
/// a longer string, it will be truncated.
#[cfg(not(test))]
pub const NOTE_FILENAME_LEN_MAX: usize = 250;
#[cfg(test)]
pub const NOTE_FILENAME_LEN_MAX: usize = 10;

/// Default content-template used when the command-line argument <sanit> is a
/// directory. Can be changed through editing the configuration file.
/// The following variables are  defined:
/// `{{ sanit | stem }}`, `{{ file | stem }}`, `{{ file | ext }}`, `{{ extension_default }}`
/// `{{ file | tag }}`, `{{ username }}`, `{{ date }}`, `{{ lang }}`,
/// `{{ path }}`.
/// In addition all environment variables can be used, e.g.
/// `{{ get_env(name=\"LOGNAME\") }}`
/// When placed in YAML-front-matter, the filter `| json_encode` must be
/// appended to each variable.
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

/// Default filename-template for a new note file on disk. It satisfies the
/// sync criteria for note-meta data in front-matter and filename.
/// Useful variables in this context are:
/// `{{ title| sanit }}`, `{{ subtitle| sanit }}`, `{{ extension_default }}`,
/// All variables also exist in a `{{ <var>| sanit(alpha) }}` variant: in case
/// its value starts with a number, the string is prepended with `'`.
/// The first non-numerical variable must be some `{{ <var>| sanit(alpha) }}`
/// variant.
/// Note, that in this filename-template, all variables must be filtered
/// by a `sanit` or `sanit(alpha=true)` filter.
const TMPL_NEW_FILENAME: &str = "\
{{ now() | date(format='%Y%m%d') }}-\
{{ title | sanit(alpha=true) }}{% if subtitle | sanit != '' %}--{% endif %}\
{{ subtitle | sanit  }}{{ extension_default | prepend_dot }}\
";

/// Default template used, when the clipboard contains a string.
/// The clipboards content is in `{{ clipboard }}`, its truncated version
/// in `{{ clipboard | heading }}`
/// When the clipboard contains a hyper-link in markdown format: [<link-name>](<link-url>),
/// its first part is stored in `{{ clipboard | linkname }}`, the second part in
/// `{{ clipboard | linkurl }}`.
/// The following variables are defined:
/// `{{ file_dirname }}`, `{{ file | stem }}`, `{{ extension }}`, `{{ extension_default }}`
/// `{{ path }}`, `{{ file | tag }}`, `{{ username }}`.
/// In addition all environment variables can be used, e.g.
/// `{{ get_env(name=\"LOGNAME\") }}`
/// When placed in YAML-front-matter, the filter `| json_encode` must be
/// appended to each variable.
/// Trick: the expression `{% if clipboard != clipboard | heading %}` detects
/// if the clipboard content has more than one line of text.
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

/// Default filename template used when the stdin ~ clipboard contains a string.
/// Useful variables in this context are:
/// `{{ title| sanit }}`, `{{ subtitle| sanit }}`, `{{ extension_default }}`,
/// All variables also exist in a `{{ <var>| sanit(alpha) }}` variant: in case
/// its value starts with a number, the string is prepended with `'`.
/// The first non-numerical variable must be some `{{ <var>| sanit(alpha) }}`
/// variant.
/// Note, that in this filename-template, all variables (except `now`)
/// must be filtered by a `sanit` or `sanit(alpha=true)` filter.
const TMPL_CLIPBOARD_FILENAME: &str = "\
{{ now() | date(format='%Y%m%d') }}-\
{{ title | sanit(alpha=true) }}{% if subtitle | sanit != '' %}--{% endif %}\
{{ subtitle | sanit  }}{{ extension_default | prepend_dot }}\
";

/// Default template used when the command-line <path> parameter points to
/// an existing non-`.md`-file. Can be modified through editing
/// the configuration file.
/// The following variables are  defined:
/// `{{ file | dirname }}`, `{{ file | stem }}`, `{{ extension }}`, `{{ extension_default }}`
/// `{{ file | tag }}`, `{{ username }}`, `{{ lang }}`,
/// `{{ path }}`.
/// In addition all environment variables can be used, e.g.
/// `{{ get_env(name=\"LOGNAME\") }}`
/// When placed in YAML-front-matter, the filter `| json_encode` must be
/// appended to each variable.
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
({{ file | tag }}{{ file | stem }}{{ file | ext | prepend_dot }})
{% if stdin ~ clipboard != '' %}{% if stdin ~ clipboard != stdin ~ clipboard | heading %}
---
{% endif %}
{{ stdin ~ clipboard }}
{% endif %}
";

/// Filename of a new note, that annotates an existing file on disk given in
/// <path>.
/// Useful variables are:
/// `{{ title | sanit(alpha=true) }}`, `{{ subtitle | sanit }}`, `{{ extension_default }}`.
/// All variables also exist in a `{{ <var>| sanit(alpha) }}` variant: in case
/// its value starts with a number, the string is prepended with `'`.
/// The first non-numerical variable must be the `{{ <var>| sanit(alpha) }}`
/// variant.
/// Note, that in this filename-template, all variables (expect `file | tag`)
/// must be filtered by a `sanit` or `sanit(alpha=true)` filter.
const TMPL_ANNOTATE_FILENAME: &str = "\
{{ file | tag }}{{ title | sanit(alpha=true) }}\
{% if subtitle | sanit != '' %}--{% endif %}\
{{ subtitle | sanit }}{{ extension_default | prepend_dot }}\
";

/// Default filename-template to test, if the filename of an existing note file on
/// disk, corresponds to the note's meta data stored in its front matter. If
/// it is not the case, the note's filename will be renamed.
/// Can be modified through editing the configuration file.
/// Useful variables in this context are:
/// `{{ tag }}`
/// `{{ title | sanit }}`, `{{ subtitle | sanit }}`, `{{ ext_default }}`,
/// All variables also exist in a `{{ <var>| sanit(alpha) }}` variant: in case
/// its value starts with a number, the string is prepended with `'`.
/// `{{ tag  }}` must be the first in line here, then followed by a
/// `{{ <var>| sanit(alpha) }}` variable.
/// Note, that in this filename-template, all variables (except `tag`) must be
/// filtered by a `sanit` or `sanit(alpha=true)` filter.
/// This is the only template that has access to the `{{ tag }}` variable.
/// `{{ tag }}` contains the content of the YAML header variable `sort_tag`.
const TMPL_SYNC_FILENAME: &str = "\
{% if sort_tag is undefined -%}{% set sort_tag = file | tag -%}{% endif -%}
{% if extension is undefined -%}{% set extension = file | ext -%}{% endif -%}
{{ sort_tag }}{{ title | sanit(alpha=true) }}\
{% if subtitle | sanit != '' %}--{% endif %}\
{{ subtitle | sanit  }}{{ extension | prepend_dot }}\
";

/// Default command-line argument list when launching external editor.
/// The editor list is executed item by item until an editor is found.
/// Can be changed in config file.
#[cfg(all(target_family = "unix", not(target_vendor = "apple")))]
const EDITOR_ARGS: &[&[&str]] = &[
    &["flatpak", "run", "com.github.marktext.marktext"],
    &["marktext", "--no-sandbox"],
    &["typora"],
    &["code", "-w", "-n"],
    &["atom", "-w"],
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

/// Default command-line argument list when launching external viewer
/// with `--view`. Can be changed in config file.
/// The viewer list is executed item by item until an editor is found.
#[cfg(all(target_family = "unix", not(target_vendor = "apple")))]
const VIEWER_ARGS: &[&[&str]] = &[
    &["marktext", "--no-sandbox"],
    &["typora"],
    &["code", "-w", "-n"],
    &["atom", "-w"],
    &["retext"],
    &["geany", "-r", "-s", "-i", "-m"],
    &["gedit", "-w"],
    &["mousepad"],
    &["leafpad"],
    &["nvim-qt", "--nofork", "-R"],
    &["gvim", "--nofork", "-R"],
];
#[cfg(target_family = "windows")]
const VIEWER_ARGS: &[&[&str]] = &[
    &["C:\\Program Files\\Mark Text\\Mark Text.exe"],
    &["C:\\Program Files\\Typora\\Typora.exe"],
    &[
        "C:\\Program Files\\Notepad++\\notepad++.exe",
        "-nosession",
        "-multiInst",
        "-ro",
    ],
    &["C:\\Windows\\notepad.exe"],
];
// Some info about lauching programs on iOS:
//[dshell.pdf](https://www.stata.com/manuals13/dshell.pdf)
#[cfg(all(target_family = "unix", target_vendor = "apple"))]
const VIEWER_ARGS: &[&[&str]] = &[
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

/// Default command-line argument list when launching external viewer
/// with `--view`. Can be changed in config file.
/// The viewer list is executed item by item until an editor is found.
#[cfg(all(target_family = "unix", not(target_vendor = "apple")))]
const VIEWER_CONSOLE_ARGS: &[&[&str]] = &[
    &["nano", "-v"],
    &["nvim", "-R"],
    &["vim", "-R"],
    &["vi", "-R"],
];
#[cfg(target_family = "windows")]
const VIEWER_CONSOLE_ARGS: &[&[&str]] = &[];
// Some info about lauching programs on iOS:
//[dshell.pdf](https://www.stata.com/manuals13/dshell.pdf)
#[cfg(all(target_family = "unix", target_vendor = "apple"))]
const VIEWER_CONSOLE_ARGS: &[&[&str]] = &[
    &["nano", "-v"],
    &["nvim", "-R"],
    &["vim", "-R"],
    &["vi", "-R"],
];

/// By default clipboard support is enabled, can be disabled
/// in config file. A false value here will set ENABLE_EMPTY_CLIPBOARD to
/// false.
const ENABLE_READ_CLIPBOARD: bool = true;

/// Should the clipboard be emptied when tp-note closes?
/// Default value.
const ENABLE_EMPTY_CLIPBOARD: bool = true;

/// Limit the size of clipboard data `tp-note` accepts as input.
const CLIPBOARD_LEN_MAX: usize = 0x8000;

#[derive(Debug, PartialEq, StructOpt)]
#[structopt(
    name = "Tp-Note",
    about = "Fast note taking with templates and filename synchronization."
)]
/// `tp-note` is a note-taking-tool and a template system, that consistently
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
    /// Launches editor in read-only mode
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
    pub version: String,
    pub extension_default: String,
    pub note_file_extensions: Vec<String>,
    pub tmpl_new_content: String,
    pub tmpl_new_filename: String,
    pub tmpl_clipboard_content: String,
    pub tmpl_clipboard_filename: String,
    pub tmpl_annotate_content: String,
    pub tmpl_annotate_filename: String,
    pub tmpl_sync_filename: String,
    pub editor_args: Vec<Vec<String>>,
    pub viewer_args: Vec<Vec<String>>,
    pub editor_console_args: Vec<Vec<String>>,
    pub viewer_console_args: Vec<Vec<String>>,
    pub enable_read_clipboard: bool,
    pub enable_empty_clipboard: bool,
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
            note_file_extensions: NOTE_FILE_EXTENSIONS
                .iter()
                .map(|a| (*a).to_string())
                .collect(),
            tmpl_new_content: TMPL_NEW_CONTENT.to_string(),
            tmpl_new_filename: TMPL_NEW_FILENAME.to_string(),
            tmpl_clipboard_content: TMPL_CLIPBOARD_CONTENT.to_string(),
            tmpl_clipboard_filename: TMPL_CLIPBOARD_FILENAME.to_string(),
            tmpl_annotate_content: TMPL_ANNOTATE_CONTENT.to_string(),
            tmpl_annotate_filename: TMPL_ANNOTATE_FILENAME.to_string(),
            tmpl_sync_filename: TMPL_SYNC_FILENAME.to_string(),
            editor_args: EDITOR_ARGS
                .iter()
                .map(|i| i.iter().map(|a| (*a).to_string()).collect())
                .collect(),
            viewer_args: VIEWER_ARGS
                .iter()
                .map(|i| i.iter().map(|a| (*a).to_string()).collect())
                .collect(),
            editor_console_args: EDITOR_CONSOLE_ARGS
                .iter()
                .map(|i| i.iter().map(|a| (*a).to_string()).collect())
                .collect(),
            viewer_console_args: VIEWER_CONSOLE_ARGS
                .iter()
                .map(|i| i.iter().map(|a| (*a).to_string()).collect())
                .collect(),
            enable_read_clipboard: ENABLE_READ_CLIPBOARD,
            enable_empty_clipboard: ENABLE_EMPTY_CLIPBOARD,
        }
    }
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
        ).unwrap_or_else(|e| {
            AlertDialog::print_error(&format!(
               "ERROR: unable to load, parse or write the configuration file\n\
                ---\n\
                \t{}\n\
                \n\
                Note: this error may occur after upgrading Tp-Note due\n\
                to some incompatible configuration file changes.\n\
                \n\
                Remedy: backup and delete the configuration file in order\n\
                to restart Tp-Note with its default configuration.", e));
            process::exit(1);
        }
    );
}

lazy_static! {
/// This is where the `confy` crate stores the configuration file.
    pub static ref CONFIG_PATH : PathBuf = {
        if let Some(c) = &ARGS.config {
            PathBuf::from(c)
        } else {
            let config = ProjectDirs::from("rs", "", CURRENT_EXE).unwrap_or_else(|| {
                AlertDialog::print_error("ERROR:\n\
                    unable to determine the configuration file directory.");
                process::exit(1)
            });
            let mut config = PathBuf::from(config.config_dir());
            config.push(Path::new(CONFIG_FILENAME));
            config
        }
    };
}

lazy_static! {
    /// Reads the input stream stdin if there is any.
    pub static ref STDIN: String = {
        let mut buffer = String::new();

        // Read stdin().
        if !is(Stream::Stdin) {
            let stdin = io::stdin();
            let mut handle = stdin.lock();
            let _ = handle.read_to_string(&mut buffer);
        }

        buffer.trim().to_string()
    };
}

lazy_static! {
    /// Reads the clipboard and empties it.
    pub static ref CLIPBOARD: String = {
        let mut buffer = String::new();

        // Concatenate clipboard content.
        if CFG.enable_read_clipboard && !*RUNS_ON_CONSOLE && !ARGS.batch {
            let ctx: Option<ClipboardContext> = ClipboardProvider::new().ok();
            if ctx.is_some() {
                let ctx = &mut ctx.unwrap(); // This is ok since `is_some()`
                let s = ctx.get_contents().ok();
                if let Some(s) = &s {
                    if s.len() > CLIPBOARD_LEN_MAX {
                        AlertDialog::print(&format!(
                            "WARNING: the clipboard content is discarded because its size \
                            exceeds {} bytes.", CLIPBOARD_LEN_MAX));
                        return String::new();
                    }
                };
                buffer.push_str(&s.unwrap_or_default());
            }
        };
        buffer.trim().to_string()
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
