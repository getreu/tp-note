//! Collects `tp-note`'s configuration from a configuration file,
//! the command-line parameters. It also reads the clipboard.

extern crate clipboard;
use anyhow::anyhow;
use lazy_static::lazy_static;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::process;
use structopt::StructOpt;

use crate::print_message;
use clipboard::ClipboardContext;
use clipboard::ClipboardProvider;

/// Name of this executable (without ".exe" extension on Windows).
const CURRENT_EXE: &str = "tp-note";

/// File extension of `to-note` files.
const NOTE_EXTENSION: &str = "md";

/// Default filename-template to test if the filename of an existing note file on
/// disk, is corresponds to the note's meta data stored in its front matter. If
/// it is not the case, the note's filename will be renamed.
/// Can be modified through editing the configuration file.
/// Useful variables in this context are:
/// `{{ sort_tag_path }}`
/// `{{ title__path }}`, `{{ subtitle__path }}`, `{{ note_extension__path }}`,
/// All variables also exist in a `{{ <var>__alphapath }}` variant: in case
/// its value starts with a number, the string is prepended with `'`.
/// `{{ sort_tag_path }}` must be the first in line here, then followed by a
/// `{{ <var>__alphapath }}` variable.
/// The first non-numerical variable must be the `{{ <var>__alphapath }}`
/// variant.
const TMPL_SYNC_FILENAME: &str = "\
{{ sort_tag__path }}\
{% if sort_tag__path != '' %}-{% endif %}\
{{ title__alphapath }}{% if subtitle__path != '' %}--{% endif %}\
{{ subtitle__path ~  '.' ~ note_extension__path }}\
";

/// Default content-template used when the command-line argument <path> is a
/// directory. Can be changed through editing the configuration file.
/// The following variables are  defined:
/// `{{ dirname }}`, `{{ file_stem }}`, `{{ extension }}`, `{{ note-extension }}`
/// `{{ sort_tag }}`, `{{ username }}`, `{{ date }}`, `{{ lang }}`,
/// `{{ path }}`, `{{ separator__path }}`, `{{ year }}`, `{{ month }}`.
/// `{{ day }}`.
/// In addition all environment variables can be used, e.g. `{{ LOGNAME }}`
/// When placed in YAML-front-matter, the filter `| json_encode()` must be
/// appended.
const TMPL_NEW_CONTENT: &str = "\
---
title:      {{ dirname | json_encode() }}
subtitle:   {{ 'Note' | json_encode() }}
author:     {{ username | json_encode() }}
date:       {{ now() | date(format=\"%Y-%m-%d\") | json_encode() }}
lang:       {{ lang | json_encode() }}
revision:   {{ '1.0' | json_encode() }}
---


";

/// Default filename-template for a new note file on disk. It satisfies the
/// sync criteria for note-meta data in front-matter and filename.
/// Useful variables in this context are:
/// `{{ title__path }}`, `{{ subtitle__path }}`, `{{ note_extension__path }}`,
/// All variables also exist in a `{{ <var>__alphapath }}` variant: in case
/// its value starts with a number, the string is prepended with `'`.
/// The first non-numerical variable must be the `{{ <var>__alphapath }}`
/// variant.
const TMPL_NEW_FILENAME: &str = "\
{{ now() | date(format=\"%Y%m%d\") }}-\
{{ title__alphapath }}{% if subtitle__path != '' %}--{% endif %}\
{{ subtitle__path ~  '.' ~ note_extension__path }}\
";

/// Default template used, when the clipboard contains a string.
/// When string represents a link in markdown format: [<link-name>](<link-url>),
/// the first part is stored in `{{ clipboard-linkname }}`, the second part in
/// `{{ clipboard-linkurl }}`.
/// The following variables are defined:
/// `{{ dirname }}`, `{{ file_stem }}`, `{{ extension }}`, `{{ note-extension }}`
/// `{{ path }}`, `{{ separator__path }}`.
/// In addition all environment variables can be used, e.g. `{{ LOGNAME }}`
/// When placed in YAML-front-matter, the filter `| json_encode()` must be
/// appended.
const TMPL_CLIPBOARD_CONTENT: &str = "\
---
{% if clipboard_linkname !='' %}title:      {{ clipboard_linkname | json_encode }}
subtitle:   {{ 'URL' | json_encode() }}
{% else %}title:      {{ clipboard | json_encode }}
subtitle:   {{ 'Note' | json_encode() }}
{% endif %}author:     {{ username | json_encode() }}
date:       {{ now() | date(format=\"%Y-%m-%d\") | json_encode() }}
lang:       {{ lang | json_encode() }}
revision:   {{ '1.0' | json_encode() }}
---

{% if clipboard_linkname !='' %}{{ clipboard }}
{% endif %}
";

/// Default filename template used when the clipboard contains a string.
/// Useful variables in this context are:
/// `{{ title__path }}`, `{{ subtitle__path }}`, `{{ note_extension__path }}`,
/// `{{ year__path }}`, `{{ month__path }}`. `{{ day__path }}`.
/// All variables also exist in a `{{ <var>__alphapath }}` variant: in case
/// its value starts with a number, the string is prepended with `'`.
/// The first non-numerical variable must be the `{{ <var>__alphapath }}`
/// variant.
const TMPL_CLIPBOARD_FILENAME: &str = "\
{{ now() | date(format=\"%Y%m%d\") }}-\
{{ title__alphapath }}{% if subtitle__path != '' %}--{% endif %}\
{{ subtitle__path ~  '.' ~ note_extension__path }}\
";

/// Default template used when the command-line <path> parameter points to
/// an existing non-`.md`-file. Can be modified through editing
/// the configuration file.
/// The following variables are  defined:
/// `{{ dirname }}`, `{{ file_stem }}`, `{{ extension }}`, `{{ note-extension }}`
/// `{{ sort_tag }}`, `{{ username }}`, `{{ lang }}`,
/// `{{ path }}`, `{{ separator__path }}`.
/// In addition all environment variables can be used, e.g. `{{ LOGNAME }}`
/// When placed in YAML-front-matter, the filter `| json_encode()` must be
/// appended.
const TMPL_ANNOTATE_CONTENT: &str = "\
---
title:      {{ sort_tag ~ file_stem | json_encode() }}
subtitle:   {{ 'Note' | json_encode() }}
author:     {{ username | json_encode() }}
date:       {{ now() | date(format=\"%Y-%m-%d\") | json_encode() }}
lang:       {{ lang | json_encode() }}
revision:   {{ '1.0' | json_encode() }}
---

[{{ sort_tag ~ file_stem ~ '.' ~ extension }}\
]({{ sort_tag ~ file_stem ~ '.' ~ extension }})

";

/// Filename of a new note, that annotates an existing file on disk given in
/// <path>.
/// Useful variables are:
/// `{{ title__path }}`, `{{ subtitle__path }}`, `{{ note_extension__path }}`.
/// All variables also exist in a `{{ <var>__alphapath }}` variant: in case
/// its value starts with a number, the string is prepended with `'`.
/// The first non-numerical variable must be the `{{ <var>__alphapath }}`
/// variant.
const TMPL_ANNOTATE_FILENAME: &str = "\
{{ title__alphapath }}{% if subtitle__path != '' %}--{% endif %}\
{{ subtitle__path ~  '.' ~ note_extension__path }}\
";

/// Default command-line argument list when launching external editor.
/// The editor list is executed item by item until an editor is found.
/// Can be changed in config file.
#[cfg(target_family = "unix")]
const EDITOR_ARGS: &[&[&str]] = &[
    &[&"typora"],
    &[&"code", &"-w"],
    &[&"atom", &"-w"],
    &[&"retext"],
    &[&"geany"],
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
    &[&"code", &"-w"],
    &[&"atom", &"-w"],
    &[&"retext"],
    &[&"geany", &"-r"],
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

/// Limit the size of clipboard data `tp-note` accepts.  As the clipboard data will be copied in
/// title by template, we better limit the length here, than having the Os complain about too long
/// filenames. Anyway, titles and filenames should not be so long.  [http - What is the maximum
/// length of a URL in different browsers? - Stack
/// Overflow](https://stackoverflow.com/questions/417142/what-is-the-maximum-length-of-a-url-in-different-browsers)
const CLIPBOARD_LEN_MAX: usize = 2048;

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
    /// Prints version and exit
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
    pub note_extension: String,
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
        Cfg {
            note_extension: NOTE_EXTENSION.to_string(),
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
    pub static ref CFG: Cfg =
        confy::load::<Cfg>(PathBuf::from(
            if ARGS.config.is_none() {
                CURRENT_EXE
            } else {
                &ARGS.config.as_ref().unwrap()
            })
            // strip extension, ".toml" is added by `confy.load()`
            .with_extension("")
            .to_str()
            .unwrap()
        ).unwrap_or_else(|e| {
            print_message(&format!("Application error: \
                unable to load/write config file: {}", e)).unwrap();
            process::exit(1)
        });
}

lazy_static! {
    /// Reads the clipboard and empties it.
    pub static ref CLIPBOARD: Option<String> = {
        if CFG.enable_read_clipboard {
            let ctx: Option<ClipboardContext> = ClipboardProvider::new().ok();
            if ctx.is_some() {
                let ctx = &mut ctx.unwrap();
                let s = ctx.get_contents().ok();
                if s.is_some() && s.as_ref().unwrap().len() > CLIPBOARD_LEN_MAX {
                    print_message(&format!(
                        "Warning: clipboard content ignored because its size \
                        exceeds {} bytes.", CLIPBOARD_LEN_MAX)).unwrap();
                    return None;
                }
                s
            } else {
                None
            }
        } else {
            None
        }
    };
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
        let mut linkname = String::new();
        let mut linkurl = String::new();

        if !input.is_empty() {
            // parse input_linkname
            let input_linkname_start = input
                .find('[')
                .ok_or_else(|| anyhow!(format!("no `[` in \"{}\"", input)))?;
            let input_linkname_end = input
                .find(']')
                .ok_or_else(|| anyhow!(format!("no `]` in \"{}\"", input)))?;
            if input_linkname_start < input_linkname_end {
                linkname = input[input_linkname_start + 1..input_linkname_end].to_string();
            // dbg!(input_linkname);
            } else {
                return Err(anyhow!(format!("no `[...]` in \"{}\"", input)));
            };

            // parse input_linkurl
            let input = &input[input_linkname_end + 1..];
            let input_linkurl_start = input
                .find('(')
                .ok_or_else(|| anyhow!(format!("no `(` in \"{}\"", input)))?;
            let input_linkurl_end = input
                .find(')')
                .ok_or_else(|| anyhow!(format!("no `)` in \"{}\"", input)))?;
            if input_linkurl_start < input_linkurl_end {
                linkurl = input[input_linkurl_start + 1..input_linkurl_end].to_string();
            // dbg!(input_linkurl);
            } else {
                return Err(anyhow!(format!("no `(...)` in \"{}\"", input)));
            }
        };

        Ok(Hyperlink {
            name: linkname,
            url: linkurl,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::Hyperlink;

    #[test]
    fn test_parse_hyperlink() {
        // Regular link
        let input = "[Homepage](https://getreu.net)";
        let expected_output = Hyperlink {
            name: "Homepage".to_string(),
            url: "https://getreu.net".to_string(),
        };

        let output = Hyperlink::new(input);

        assert_eq!(expected_output, output.unwrap());

        // link with () in name
        let input = "[Homepage (my first)](https://getreu.net)";
        let expected_output = Hyperlink {
            name: "Homepage (my first)".to_string(),
            url: "https://getreu.net".to_string(),
        };

        let output = Hyperlink::new(input);

        assert_eq!(expected_output, output.unwrap());

        // link with only []
        let input = "[Homepage (my first)]";

        let output = Hyperlink::new(input);

        assert!(output.is_err());
    }
}
