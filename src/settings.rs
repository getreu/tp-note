//! Reads the command line parameters and clipboard and exposes them as `static`
//! variables.

#[cfg(any(feature = "read-clipboard", feature = "viewer"))]
use crate::config::CFG;
use crate::content::Content;
use crate::error::NoteError;
use atty::{is, Stream};
#[cfg(feature = "read-clipboard")]
use clipboard::ClipboardContext;
#[cfg(feature = "read-clipboard")]
use clipboard::ClipboardProvider;
use lazy_static::lazy_static;
use log::LevelFilter;
use parse_hyperlinks::iterator::first_hyperlink;
use std::io;
use std::io::Read;
use std::path::PathBuf;
use structopt::StructOpt;

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
    /// Console debug level: "trace", "debug", "info", "warn", "error" (default) or "off"
    #[structopt(long, short = "d")]
    pub debug: Option<LevelFilter>,
    /// Show console debug messages also as popup windows
    #[structopt(long, short = "u")]
    pub popup: bool,
    /// Launches only the editor, no browser
    #[structopt(long, short = "e")]
    pub edit: bool,
    /// Lets web server listen to a specific port
    #[structopt(long, short = "p")]
    pub port: Option<u16>,
    /// Disables filename synchronization
    #[structopt(long, short = "n")]
    pub no_filename_sync: bool,
    /// Launches only the browser, no editor
    #[structopt(long, short = "v")]
    pub view: bool,
    /// <dir> as new note location or <file> to annotate
    #[structopt(name = "PATH", parse(from_os_str))]
    pub path: Option<PathBuf>,
    /// Prints version and exits
    #[structopt(long, short = "V")]
    pub version: bool,
    /// Saves the HTML rendition in the <export>
    /// dir, the note's dir if '' or stdout if '-'.
    #[structopt(long, short = "x", parse(from_os_str))]
    pub export: Option<PathBuf>,
}

lazy_static! {
/// Structure to hold the parsed command line arguments.
pub static ref ARGS : Args = Args::from_args();
}

lazy_static! {
    /// Shall we launch the external text editor?
    pub static ref LAUNCH_EDITOR: bool = {
        !ARGS.batch && ARGS.export.is_none() && !ARGS.view
    };
}

#[cfg(feature = "viewer")]
lazy_static! {
    /// Shall we launch the internal http server and the external browser?
    pub static ref LAUNCH_VIEWER: bool = {
        !ARGS.batch && ARGS.export.is_none() && !*RUNS_ON_CONSOLE &&
            (ARGS.view || ( !ARGS.edit && !CFG.arg_default.edit ))
    };
}

#[cfg(not(feature = "viewer"))]
lazy_static! {
    /// Shall we launch the internal http server and the external browser?
    pub static ref LAUNCH_VIEWER: bool = {
        false
    };
}

lazy_static! {
    /// Do we run on a console?
    pub static ref RUNS_ON_CONSOLE: bool = {
        // User `root` has usually no GUI.
        #[cfg(target_family = "unix")]
        if let Some(user) = std::env::var("USER")
            // Map error to `None`.
            .ok()
            // A pattern mapping `Some("")` to `None`.
            .and_then(|s: String| if s.is_empty() { None } else { Some(s) }) {
            if user == "root" {
                return true;
            }
        }

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
    /// Reads the input stream stdin if there is any.
    pub static ref STDIN: Content = {
        let mut buffer = String::new();

        // Read stdin().
        if !is(Stream::Stdin) {
            let stdin = io::stdin();
            let mut handle = stdin.lock();
            let _ = handle.read_to_string(&mut buffer);
        }

        // `trim_end()` content without new allocation.
        buffer.truncate(buffer.trim_end().len());

        Content::from_input_with_cr(buffer)
    };
}

lazy_static! {
    /// Reads the clipboard, if there is any and empties it.
    pub static ref CLIPBOARD: Content = {
        let mut buffer = String::new();

        // Concatenate clipboard content.
        #[cfg(feature="read-clipboard")]
        if CFG.clipboard.read_enabled && !*RUNS_ON_CONSOLE && !ARGS.batch {
            let ctx: Option<ClipboardContext> = ClipboardProvider::new().ok();
            if ctx.is_some() {
                let ctx = &mut ctx.unwrap(); // This is ok since `is_some()>`
                let s = ctx.get_contents().ok();
                buffer.push_str(&s.unwrap_or_default());
            }
        };

        // `trim_end()` content without new allocation.
        buffer.truncate(buffer.trim_end().len());

        Content::from_input_with_cr(buffer)
    };
}

#[derive(Debug, PartialEq, Default)]
/// Represents a hyperlink.
pub struct Hyperlink {
    pub name: String,
    pub target: String,
    pub title: String,
}

impl Hyperlink {
    /// Parse a markdown formatted hyperlink and stores the result in `Self`.
    pub fn new(input: &str) -> Result<Hyperlink, NoteError> {
        if let Some((link_name, link_target, link_title)) = first_hyperlink(input) {
            Ok(Hyperlink {
                name: link_name.to_string(),
                target: link_target.to_string(),
                title: link_title.to_string(),
            })
        } else {
            Err(NoteError::NoHyperlinkFound)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::Hyperlink;

    #[test]
    fn test_parse_hyperlink() {
        // Stand alone Markdown link.
        let input = r#"abc[Homepage](https://blog.getreu.net "My blog")abc"#;
        let expected_output = Hyperlink {
            name: "Homepage".to_string(),
            target: "https://blog.getreu.net".to_string(),
            title: "My blog".to_string(),
        };
        let output = Hyperlink::new(input);
        assert_eq!(expected_output, output.unwrap());

        // Markdown link refernce.
        let input = r#"abc[Homepage][home]abc
                      [home]: https://blog.getreu.net "My blog""#;
        let expected_output = Hyperlink {
            name: "Homepage".to_string(),
            target: "https://blog.getreu.net".to_string(),
            title: "My blog".to_string(),
        };
        let output = Hyperlink::new(input);
        assert_eq!(expected_output, output.unwrap());

        //
        // RestructuredText link
        let input = "abc`Homepage <https://blog.getreu.net>`_\nabc";
        let expected_output = Hyperlink {
            name: "Homepage".to_string(),
            target: "https://blog.getreu.net".to_string(),
            title: "".to_string(),
        };
        let output = Hyperlink::new(input);
        assert_eq!(expected_output, output.unwrap());

        //
        // RestructuredText link ref
        let input = "abc `Homepage<home_>`_ abc\n.. _home: https://blog.getreu.net\nabc";
        let expected_output = Hyperlink {
            name: "Homepage".to_string(),
            target: "https://blog.getreu.net".to_string(),
            title: "".to_string(),
        };
        let output = Hyperlink::new(input);
        assert_eq!(expected_output, output.unwrap());
    }
}
