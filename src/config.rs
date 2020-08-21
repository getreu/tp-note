//! Collects `tp-note`'s configuration from a configuration file,
//! the command-line parameters. It also reads the clipboard.

extern crate clipboard;
extern crate directories;
use crate::MESSAGE_ALERT_WINDOW_TITLE;
use crate::VERSION;
use anyhow::anyhow;
use clipboard::ClipboardContext;
use clipboard::ClipboardProvider;
use directories::ProjectDirs;
use lazy_static::lazy_static;
use msgbox::IconType;
use serde::{Deserialize, Serialize};
use std::path::Path;
use std::path::PathBuf;
use std::process;
use structopt::StructOpt;

/// Name of this executable (without the Windows ".exe" extension).
const CURRENT_EXE: &str = "tp-note";

/// Crate `confy` version 0.4 uses this filename by default.
const CONFIG_FILENAME: &str = "tp-note.toml";

/// File extension of `to-note` files.
const EXTENSION_DEFAULT: &str = "md";

/// Maximum length of a note's filename in bytes. If a filename-template produces
/// a longer string, it will be truncated.
#[cfg(not(test))]
pub const NOTE_FILENAME_LEN_MAX: usize = 250;
#[cfg(test)]
pub const NOTE_FILENAME_LEN_MAX: usize = 10;

/// Default filename-template to test, if the filename of an existing note file on
/// disk, corresponds to the note's meta data stored in its front matter. If
/// it is not the case, the note's filename will be renamed.
/// Can be modified through editing the configuration file.
/// Useful variables in this context are:
/// `{{ tag }}`
/// `{{ title | path }}`, `{{ subtitle | path }}`, `{{ extension_default | path }}`,
/// All variables also exist in a `{{ <var>| path(alpha) }}` variant: in case
/// its value starts with a number, the string is prepended with `'`.
/// `{{ tag  }}` must be the first in line here, then followed by a
/// `{{ <var>| path(alpha) }}` variable.
/// Note, that in this filename-template, all variables (except `tag`) must be
/// filtered by a `path` or `path(alpha=true)` filter.
/// This is the only template that has access to the `{{ tag }}` variable.
/// `{{ tag }}` contains the content of the YAML header variable `tag:` if
/// it exists. Otherwise it defaults to `{{ file_tag }}`.

const TMPL_SYNC_FILENAME: &str = "\
{{ tag }}\
{{ title | path(alpha=true) }}{% if subtitle | path != '' %}--{% endif %}\
{{ subtitle | path  }}.{{ file_extension | path }}\
";

/// Default content-template used when the command-line argument <path> is a
/// directory. Can be changed through editing the configuration file.
/// The following variables are  defined:
/// `{{ file_dirname }}`, `{{ file_stem }}`, `{{ file_extension }}`, `{{ extension_default }}`
/// `{{ file_tag }}`, `{{ username }}`, `{{ date }}`, `{{ lang }}`,
/// `{{ path }}`.
/// In addition all environment variables can be used, e.g.
/// `{{ get_env(name=\"LOGNAME\") }}`
/// When placed in YAML-front-matter, the filter `| json_encode` must be
/// appended to each variable.
const TMPL_NEW_CONTENT: &str = "\
---
title:      {{ file_dirname | json_encode }}
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
/// `{{ title| path }}`, `{{ subtitle| path }}`, `{{ extension_default| path }}`,
/// All variables also exist in a `{{ <var>| path(alpha) }}` variant: in case
/// its value starts with a number, the string is prepended with `'`.
/// The first non-numerical variable must be some `{{ <var>| path(alpha) }}`
/// variant.
/// Note, that in this filename-template, all variables must be filtered
/// by a `path` or `path(alpha=true)` filter.
const TMPL_NEW_FILENAME: &str = "\
{{ now() | date(format='%Y%m%d') }}-\
{{ title | path(alpha=true) }}{% if subtitle | path != '' %}--{% endif %}\
{{ subtitle | path  }}.{{ extension_default | path }}\
";

/// Default template used, when the clipboard contains a string.
/// The clipboards content is in `{{ clipboard }}`, its truncated version
/// in `{{ clipboard_heading }}`
/// When the clipboard contains a hyper-link in markdown format: [<link-name>](<link-url>),
/// its first part is stored in `{{ clipboard-linkname }}`, the second part in
/// `{{ clipboard-linkurl }}`.
/// The following variables are defined:
/// `{{ file_dirname }}`, `{{ file_stem }}`, `{{ extension }}`, `{{ extension_default }}`
/// `{{ path }}`, `{{ file_tag }}`, `{{ username }}`.
/// In addition all environment variables can be used, e.g.
/// `{{ get_env(name=\"LOGNAME\") }}`
/// When placed in YAML-front-matter, the filter `| json_encode` must be
/// appended to each variable.
const TMPL_CLIPBOARD_CONTENT: &str = "\
---
{% if clipboard_linkname !='' %}title:      {{ clipboard_linkname | json_encode }}
subtitle:   {{ 'URL' | json_encode }}
{% else %}title:      {{ clipboard_heading | json_encode }}
subtitle:   {{ 'Note' | json_encode }}
{% endif %}author:     {{ username | json_encode }}
date:       {{ now() | date(format='%Y-%m-%d') | json_encode }}
lang:       {{ get_env(name='LANG', default='') | json_encode }}
revision:   {{ '1.0' | json_encode }}
---

{{ clipboard }}

";

/// Default filename template used when the clipboard contains a string.
/// Useful variables in this context are:
/// `{{ title| path }}`, `{{ subtitle| path }}`, `{{ extension_default| path }}`,
/// `{{ year| path }}`, `{{ month| path }}`. `{{ day| path }}`.
/// All variables also exist in a `{{ <var>| path(alpha) }}` variant: in case
/// its value starts with a number, the string is prepended with `'`.
/// The first non-numerical variable must be some `{{ <var>| path(alpha) }}`
/// variant.
/// Note, that in this filename-template, all variables (except `now`)
/// must be filtered by a `path` or `path(alpha=true)` filter.
const TMPL_CLIPBOARD_FILENAME: &str = "\
{{ now() | date(format='%Y%m%d') }}-\
{{ title | path(alpha=true) }}{% if subtitle | path != '' %}--{% endif %}\
{{ subtitle | path  }}.{{ extension_default | path }}\
";

/// Default template used when the command-line <path> parameter points to
/// an existing non-`.md`-file. Can be modified through editing
/// the configuration file.
/// The following variables are  defined:
/// `{{ file_dirname }}`, `{{ file_stem }}`, `{{ extension }}`, `{{ extension_default }}`
/// `{{ file_tag }}`, `{{ username }}`, `{{ lang }}`,
/// `{{ path }}`.
/// In addition all environment variables can be used, e.g.
/// `{{ get_env(name=\"LOGNAME\") }}`
/// When placed in YAML-front-matter, the filter `| json_encode` must be
/// appended to each variable.
const TMPL_ANNOTATE_CONTENT: &str = "\
---
title:      {{ file_stem | json_encode }}
subtitle:   {{ 'Note' | json_encode }}
author:     {{ username | json_encode }}
date:       {{ now() | date(format='%Y-%m-%d') | json_encode }}
lang:       {{ get_env(name='LANG', default='') | json_encode }}
revision:   {{ '1.0' | json_encode }}
---

[{{ file_tag ~ file_stem ~ '.' ~ file_extension }}\
]({{ file_tag ~ file_stem ~ '.' ~ file_extension }})

";

/// Filename of a new note, that annotates an existing file on disk given in
/// <path>.
/// Useful variables are:
/// `{{ title | path(alpha=true) }}`, `{{ subtitle | path }}`, `{{ extension_default | path }}`.
/// All variables also exist in a `{{ <var>| path(alpha) }}` variant: in case
/// its value starts with a number, the string is prepended with `'`.
/// The first non-numerical variable must be the `{{ <var>| path(alpha) }}`
/// variant.
/// Note, that in this filename-template, all variables (expect `file_tag`)
/// must be filtered by a `path` or `path(alpha=true)` filter.
const TMPL_ANNOTATE_FILENAME: &str = "\
{{ file_tag }}\
{{ title | path(alpha=true) }}{% if subtitle | path != '' %}--{% endif %}\
{{ subtitle | path  }}.{{ extension_default | path }}\
";

/// Default command-line argument list when launching external editor.
/// The editor list is executed item by item until an editor is found.
/// Can be changed in config file.
#[cfg(target_family = "unix")]
const EDITOR_ARGS: &[&[&str]] = &[
    &[&"typora"],
    &[&"code", &"-w", &"-n"],
    &[&"atom", &"-w"],
    &[&"retext"],
    &[&"geany", &"-s", &"-i", &"-m"],
    &[&"gedit", &"-w"],
    &[&"mousepad"],
    &[&"leafpad"],
    &[&"nvim-qt", &"--nofork"],
    &[&"gvim", &"--nofork"],
    &[&"nano"],
    &[&"nvim"],
    &[&"vim"],
    &[&"vi"],
];
#[cfg(target_family = "windows")]
const EDITOR_ARGS: &[&[&str]] = &[
    &[&"C:\\Program Files\\Typora\\Typora.exe"],
    &[
        "C:\\Program Files\\Notepad++\\notepad++.exe",
        "-nosession",
        "-multiInst",
    ],
    &[&"C:\\Windows\\notepad.exe"],
];
// Some info about lauching programs on iOS:
//[dshell.pdf](https://www.stata.com/manuals13/dshell.pdf)
#[cfg(target_os = "ios")]
const EDITOR_ARGS: &[&[&str]] = &[&[&"/Applications/TextEdit.app/Contents/MacOS/TextEdit"]];

/// Default command-line argument list when launching external viewer
/// with `--view`. Can be changed in config file.
/// The viewer list is executed item by item until an editor is found.
#[cfg(target_family = "unix")]
const VIEWER_ARGS: &[&[&str]] = &[
    &[&"typora"],
    &[&"code", &"-w", &"-n"],
    &[&"atom", &"-w"],
    &[&"retext"],
    &[&"geany", &"-r", &"-s", &"-i", &"-m"],
    &[&"gedit", &"-w"],
    &[&"mousepad"],
    &[&"leafpad"],
    &[&"nvim-qt", &"--nofork", &"-R"],
    &[&"gvim", &"--nofork", &"-R"],
    &[&"nvim", &"-R"],
    &[&"nano"],
    &[&"nvim", &"-R"],
    &[&"vim", &"-R"],
    &[&"vi", &"-R"],
];
#[cfg(target_family = "windows")]
const VIEWER_ARGS: &[&[&str]] = &[
    &[&"C:\\Program Files\\Typora\\Typora.exe"],
    &[
        "C:\\Program Files\\Notepad++\\notepad++.exe",
        "-nosession",
        "-multiInst",
        "-ro",
    ],
    &[&"C:\\Windows\\notepad.exe"],
];
// Some info about lauching programs on iOS:
//[dshell.pdf](https://www.stata.com/manuals13/dshell.pdf)
#[cfg(target_os = "ios")]
const VIEWER_ARGS: &[&[&str]] = &[&[&"/Applications/TextEdit.app/Contents/MacOS/TextEdit"]];

/// By default clipboard support is enabled, can be disabled
/// in config file. A false value here will set ENABLE_EMPTY_CLIPBOARD to
/// false.
const ENABLE_READ_CLIPBOARD: bool = true;

/// Should the clipboard be emptied when tp-note closes?
/// Default value.
const ENABLE_EMPTY_CLIPBOARD: bool = true;

/// Limit the size of clipboard data `tp-note` accepts as input.
const CLIPBOARD_LEN_MAX: usize = 0x8000;

/// Defines the maximum length of the template variables `{{ clipboard_truncated }}` and `{{
/// clipboard_linkname }}` which are usually used to in the note's front matter as title.  The
/// title should not be too long, because it will end up as part of the file-name when the note is
/// saved to disk. Filenames of some operating systems are limited to 255 bytes.
#[cfg(not(test))]
const CLIPBOARD_TRUNCATED_LEN_MAX: usize = 200;
#[cfg(test)]
const CLIPBOARD_TRUNCATED_LEN_MAX: usize = 10;

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
    pub tmpl_new_content: String,
    pub tmpl_new_filename: String,
    pub tmpl_clipboard_content: String,
    pub tmpl_clipboard_filename: String,
    pub tmpl_annotate_content: String,
    pub tmpl_annotate_filename: String,
    pub tmpl_sync_filename: String,
    pub editor_args: Vec<Vec<String>>,
    pub viewer_args: Vec<Vec<String>>,
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
            enable_read_clipboard: ENABLE_READ_CLIPBOARD,
            enable_empty_clipboard: ENABLE_EMPTY_CLIPBOARD,
        }
    }
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
            print_message(&format!(
                "Application error: unable to load/write the configuration file:\n---\n\
                Configuration file path:\n\
                \t{:?}\n\
                Error:\n\
                \t{}\n\
                ---\nBackup and delete the configuration file to restart Tp-Note \n\
                with its default configuration.", *CONFIG_PATH, e));
            process::exit(1);
        }
    );
}

lazy_static! {
/// This is where the `confy` crate stores the configuration file.
    pub static ref CONFIG_PATH : PathBuf = {
        let config = ProjectDirs::from("rs", "", CURRENT_EXE).unwrap_or_else(|| {
            print_message("Application error: \
                unable to determine the configuration file directory.");
            process::exit(1)
        });
        let mut config = PathBuf::from(config.config_dir());
        config.push(Path::new(CONFIG_FILENAME));
        config
    };
}

lazy_static! {
    /// Reads the clipboard and empties it.
    pub static ref CLIPBOARD: Clipboard = {
        if CFG.enable_read_clipboard {
            let ctx: Option<ClipboardContext> = ClipboardProvider::new().ok();
            if ctx.is_some() {
                let ctx = &mut ctx.unwrap(); // This is ok since `is_some()`
                let s = ctx.get_contents().ok();
                if let Some(s) = &s {
                    if s.len() > CLIPBOARD_LEN_MAX {
                        print_message(&format!(
                            "Warning: the clipboard content is discarded because its size \
                            exceeds {} bytes.", CLIPBOARD_LEN_MAX));
                        return Clipboard::default();
                    }
                };
                Clipboard::new(&s.unwrap_or_default())
            } else {
                Clipboard::default()
            }
        } else {
            Clipboard::default()
        }
    };

}

#[derive(Debug, PartialEq)]
/// Represents the clipboard content.
pub struct Clipboard {
    /// Raw content sting.
    pub content: String,
    /// Shortened content string (max CLIPBOARD_SHORT_LEN_MAX).
    pub content_truncated: String,
    /// First sentence (all characters until the first period)
    /// or all characters until the first empty line.
    /// If none is found take the whole `content_truncated`.
    pub content_heading: String,
    /// Namepart of the Markdown link. Empty if none.
    pub linkname: String,
    /// URL part of the Markdown link. Empty if none.
    pub linkurl: String,
}

impl Clipboard {
    pub fn new(content: &str) -> Self {
        let content: String = content.trim_start().to_string();

        // Limit the size of `clipboard_truncated`
        let mut content_truncated = String::new();
        for i in (0..CLIPBOARD_TRUNCATED_LEN_MAX).rev() {
            if let Some(s) = content.get(..i) {
                content_truncated = s.to_string();
                break;
            }
        }

        // Find the first heading, can finish with `. `, `.\n` or `.\r\n` on Windows.
        let mut index = content_truncated.len();

        if let Some(i) = content_truncated.find(". ") {
            if i < index {
                index = i;
            }
        }
        if let Some(i) = content_truncated.find(".\n") {
            if i < index {
                index = i;
            }
        }
        if let Some(i) = content_truncated.find(".\r\n") {
            if i < index {
                index = i;
            }
        }
        if let Some(i) = content_truncated.find('!') {
            if i < index {
                index = i;
            }
        }
        if let Some(i) = content_truncated.find('?') {
            if i < index {
                index = i;
            }
        }
        if let Some(i) = content_truncated.find("\n\n") {
            if i < index {
                index = i;
            }
        }
        if let Some(i) = content_truncated.find("\r\n\r\n") {
            if i < index {
                index = i;
            }
        }
        let content_heading = content_truncated[0..index].to_string();

        // Parse clipboard for markdown hyperlink.
        let hyperlink = match Hyperlink::new(&content) {
            Ok(s) => Some(s),
            Err(e) => {
                if ARGS.debug {
                    eprintln!("Note: the clipboard does not contain a markdown link: {}", e);
                }
                None
            }
        };

        let mut linkname = String::new();
        let mut linkurl = String::new();
        // If there is a hyperlink in clipboard, destructure.
        if let Some(hyperlink) = hyperlink {
            linkname = hyperlink.name.to_owned();
            linkurl = hyperlink.url.to_owned();
        };

        // Limit the size of `linkname`.
        for i in (0..CLIPBOARD_TRUNCATED_LEN_MAX).rev() {
            if let Some(s) = linkname.get(..i) {
                linkname = s.to_string();
                break;
            }
        }

        Self {
            content,
            content_truncated,
            content_heading,
            linkname,
            linkurl,
        }
    }
}

/// By default, the clipboard is empty.
impl ::std::default::Default for Clipboard {
    fn default() -> Self {
        Self {
            content: "".to_string(),
            content_truncated: "".to_string(),
            content_heading: "".to_string(),
            linkname: "".to_string(),
            linkurl: "".to_string(),
        }
    }
}

#[derive(Debug, PartialEq)]
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

/// Pops up a message box and prints `msg`.
pub fn print_message(msg: &str) {
    let title = format!(
        "{} (v{})",
        MESSAGE_ALERT_WINDOW_TITLE,
        VERSION.unwrap_or("unknown")
    );
    // Print the same message also to console in case
    // the window does not pop up due to missing
    // libraries.
    print_message_console(msg);
    // Popup window.
    msgbox::create(&title, msg, IconType::Info);
}

/// Prints `msg` on console.
pub fn print_message_console(msg: &str) {
    let title = format!(
        "{} (v{})",
        MESSAGE_ALERT_WINDOW_TITLE,
        VERSION.unwrap_or("unknown")
    );
    // Print the same message also to console in case
    // the window does not pop up due to missing
    // libraries.
    eprintln!("{}\n\n{}", title, msg);
}

#[cfg(test)]
mod tests {
    use super::Clipboard;
    use super::Hyperlink;

    #[test]
    fn test_clipboard() {
        // Test Markdown link in clipboard.
        let input = "[Jens Getreu's blog](https://blog.getreu.net)";
        let output = Clipboard::new(input);
        // This string is shortened.
        assert_eq!("Jens Getr", output.linkname);
        assert_eq!("https://blog.getreu.net", output.linkurl);
        assert_eq!(
            "[Jens Getreu's blog](https://blog.getreu.net)",
            output.content
        );

        //
        // Test non-link string in clipboard.
        let input = "Tp-Note helps you to quickly get\
            started writing notes.";
        let output = Clipboard::new(input);

        assert_eq!("", output.linkname);
        assert_eq!("", output.linkurl);
        assert_eq!(
            "Tp-Note helps you to quickly get\
            started writing notes.",
            output.content
        );
        // This string is shortened.
        assert_eq!("Tp-Note h", output.content_truncated);

        //
        // Test find heading.
        let input = "N.ote. It helps. Get quickly\
            started writing notes.";
        let output = Clipboard::new(input);

        assert_eq!("", output.linkname);
        assert_eq!("", output.linkurl);
        assert_eq!(
            "N.ote. It helps. Get quickly\
            started writing notes.",
            output.content
        );
        // This string is shortened.
        assert_eq!("N.ote", output.content_heading);

        //
        // Test find first sentence.
        let input = "N.ote.\nIt helps. Get quickly\
            started writing notes.";
        let output = Clipboard::new(input);
        // This string is shortened.
        assert_eq!("N.ote", output.content_heading);

        //
        // Test find first sentence (Windows)
        let input = "N.ote.\r\nIt helps. Get quickly\
            started writing notes.";
        let output = Clipboard::new(input);
        // This string is shortened.
        assert_eq!("N.ote", output.content_heading);

        //
        // Test find heading
        let input = "N.ote\n\nIt helps. Get quickly\
            started writing notes.";
        let output = Clipboard::new(input);
        // This string is shortened.
        assert_eq!("N.ote", output.content_heading);

        //
        // Test find heading (Windows)
        let input = "N.ote\r\n\r\nIt helps. Get quickly\
            started writing notes.";
        let output = Clipboard::new(input);
        // This string is shortened.
        assert_eq!("N.ote", output.content_heading);

        //
        // Test trim whitespace
        let input = "\r\n\r\n  \tIt helps. Get quickly\
            started writing notes.";
        let output = Clipboard::new(input);
        // This string is shortened.
        assert_eq!("It helps.", output.content_heading);
    }

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
