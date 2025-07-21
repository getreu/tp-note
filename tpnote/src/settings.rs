//! Reads the command line parameters and clipboard and exposes them as `static`
//! variables.

//#[cfg(any(feature = "read-clipboard", feature = "viewer"))]
use crate::clipboard::SystemClipboard;
use crate::config::CFG;
use clap::Parser;
use clap::ValueEnum;
use serde::Deserialize;
use serde::Serialize;
use std::env;
use std::io;
use std::io::IsTerminal;
use std::io::Read;
use std::path::PathBuf;
use std::sync::LazyLock;
use tpnote_lib::config::LocalLinkKind;
use tpnote_lib::config::TMPL_VAR_STDIN;
use tpnote_lib::content::Content;
use tpnote_lib::content::ContentString;
use tpnote_lib::text_reader::CrlfSuppressorExt;

/// The name of the environment variable, that - when set - replaces the default
/// path where Tp-Note loads or stores its configuration file.
pub const ENV_VAR_TPNOTE_CONFIG: &str = "TPNOTE_CONFIG";
/// The name of the environment variable which can be optionally set to launch
/// a different file editor.
pub const ENV_VAR_TPNOTE_EDITOR: &str = "TPNOTE_EDITOR";
/// The name of the environment variable which can be optionally used to launch
/// a different web browser.
#[cfg(feature = "viewer")]
pub const ENV_VAR_TPNOTE_BROWSER: &str = "TPNOTE_BROWSER";
/// The name of the environment variable which Tp-Note checks under Unix, if it
/// is invoked as `root`.
#[cfg(target_family = "unix")]
const ENV_VAR_USER: &str = "USER";
/// The name of the environment variable which Tp-Note checks under Unix, if it
/// is invoked on a graphical desktop.
#[cfg(target_family = "unix")]
const ENV_VAR_DISPLAY: &str = "DISPLAY";

#[derive(Debug, Eq, PartialEq, Parser)]
#[command(
    version,
    name = "Tp-Note",
    about,
    long_about = "Fast note taking with templates and filename synchronization.",
    disable_version_flag = true
)]
/// _Tp-Note_ is a note-taking tool and a template system, that synchronizes the
/// note's metadata with its filename. _Tp-Note_ collects various information
/// about its environment and the clipboard and stores it in variables. New
/// notes are created by filling these variables in predefined and customizable
/// `Tera`-templates. In case `<path>` points to an existing _Tp-Note_ file, the
/// note's metadata is analyzed and, if necessary, its filename is adjusted.
/// For all other filetypes, _Tp-Note_ creates a new note annotating the
/// file `<path>` points to. If `<path>` is a directory (or, when omitted the
/// current working directory), a new note is created in that directory. After
/// creation, _Tp-Note_ launches an external editor of your choice. Although the
/// templates are written for Markdown, _Tp-Note_ is not tied to
/// any specific markup language. However, _Tp-Note_ comes with an optional
/// viewer feature, that currently renders only Markdown, ReStructuredText and
/// HTML. Note, that there is also some limited support for Asciidoc and
/// WikiText. The note's rendition with its hyperlinks is live updated and
/// displayed in the user's web browser.
pub struct Args {
    /// Prepends a YAML header if missing
    #[arg(long, short = 'a')]
    pub add_header: bool,
    /// Batch mode: does not launch the editor or the viewer
    #[arg(long, short = 'b')]
    pub batch: bool,
    /// Loads (and merges) an additional configuration file
    #[arg(long, short = 'c')]
    pub config: Option<String>,
    /// Dumps the internal default configuration into a file
    /// or stdout if `-`
    #[arg(long, short = 'C')]
    pub config_defaults: Option<String>,
    /// Console debug level:
    #[arg(long, short = 'd', value_enum)]
    pub debug: Option<ClapLevelFilter>,
    /// Shows console debug messages also as popup windows
    #[arg(long, short = 'u')]
    pub popup: bool,
    /// Launches only the editor, no browser
    #[arg(long, short = 'e')]
    pub edit: bool,
    /// Scheme for new notes: "default", "zettel", (cf. `--config-defaults`)
    #[arg(long, short = 's')]
    pub scheme: Option<String>,
    /// Console mode: opens the console editor, no browser
    #[arg(long, short = 't')]
    pub tty: bool,
    /// Lets the web server listen to a specific port
    #[arg(long, short = 'p')]
    pub port: Option<u16>,
    /// Disables filename synchronization
    #[arg(long, short = 'n')]
    pub no_filename_sync: bool,
    /// Disables automatic language detection and uses `<FORCE_LANG>`
    /// instead; or, if '-' use `TPNOTE_LANG` or `LANG`
    #[arg(long, short = 'l')]
    pub force_lang: Option<String>,
    /// Launches only the browser, no editor
    #[arg(long, short = 'v')]
    pub view: bool,
    /// `<DIR>` the new note's location or `<FILE>` to open or to annotate
    #[arg(name = "PATH")]
    pub path: Option<PathBuf>,
    /// Prints the version and exits
    #[arg(long, short = 'V')]
    pub version: bool,
    /// Saves the HTML rendition in the `<EXPORT>` directory,
    /// the note's directory if '.' or standard output if '-'.
    #[arg(long, short = 'x')]
    pub export: Option<PathBuf>,
    /// Exporter local link rewriting: [possible values: off, short, long]
    #[arg(long, value_enum)]
    pub export_link_rewriting: Option<LocalLinkKind>,
}

/// Structure to hold the parsed command line arguments.
pub static ARGS: LazyLock<Args> = LazyLock::new(Args::parse);

/// Shall we launch the external text editor?
pub static LAUNCH_EDITOR: LazyLock<bool> = LazyLock::new(|| {
    !ARGS.batch
        && ARGS.export.is_none()
        && env::var(ENV_VAR_TPNOTE_EDITOR) != Ok(String::new())
        && (ARGS.edit || !ARGS.view)
});

#[cfg(feature = "viewer")]
/// Shall we launch the internal HTTP server and the external browser?
pub static LAUNCH_VIEWER: LazyLock<bool> = LazyLock::new(|| {
    !ARGS.batch
        && ARGS.export.is_none()
        && !*RUNS_ON_CONSOLE
        && (ARGS.view
            || (!ARGS.edit
                && !CFG.arg_default.edit
                && env::var(ENV_VAR_TPNOTE_BROWSER) != Ok(String::new())))
});

#[cfg(not(feature = "viewer"))]
/// Shall we launch the internal HTTP server and the external browser?
pub static LAUNCH_VIEWER: LazyLock<bool> = LazyLock::new(|| false);

/// Do we run on a console?
pub static RUNS_ON_CONSOLE: LazyLock<bool> = LazyLock::new(|| {
    use crate::CFG;
    // User `root` has usually no GUI.
    #[cfg(target_family = "unix")]
    if let Some(user) = std::env::var(ENV_VAR_USER)
        // Map error to `None`.
        .ok()
        // A pattern mapping `Some("")` to `None`.
        .and_then(|s: String| if s.is_empty() { None } else { Some(s) })
    {
        if user == "root" {
            return true;
        }
    }

    // On Linux popup window only if DISPLAY is set.
    #[cfg(target_family = "unix")]
    let display = std::env::var(ENV_VAR_DISPLAY)
        // Map error to `None`.
        .ok()
        // A pattern mapping `Some("")` to `None`.
        .and_then(|s: String| if s.is_empty() { None } else { Some(s) });
    // In non-Linux there is always "Some" display.
    #[cfg(not(target_family = "unix"))]
    let display = Some(String::new());

    display.is_none() || ARGS.tty || CFG.arg_default.tty
});

/// Reads the input stream standard input if there is any.
pub static STDIN: LazyLock<ContentString> = LazyLock::new(|| {
    // Bring new methods into scope.
    use tpnote_lib::html::HtmlStr;

    let mut buffer = String::new();

    // Read the standard input.
    let stdin = io::stdin();
    if !stdin.is_terminal() {
        // There is an input pipe for us to read from.
        let handle = stdin.lock();
        let buf = handle.bytes().crlf_suppressor();
        let buf: Result<Vec<u8>, std::io::Error> = buf.collect();
        let buf = buf.unwrap_or_default();
        buffer = String::from_utf8(buf).unwrap_or_default();
    }

    // Guess if this is an HTML stream.
    if buffer.is_html_unchecked() {
        ContentString::from_html(buffer, TMPL_VAR_STDIN.to_string()).unwrap_or_else(|e| {
            ContentString::from_string(e.to_string(), TMPL_VAR_STDIN.to_string())
        })
    } else {
        ContentString::from_string(buffer, TMPL_VAR_STDIN.to_string())
    }
});

/// Reads the clipboard, if there is any and empties it.
pub static SYSTEM_CLIPBOARD: LazyLock<SystemClipboard> = LazyLock::new(|| {
    if CFG.clipboard.read_enabled && !ARGS.batch {
        SystemClipboard::new()
    } else {
        SystemClipboard::default()
    }
});

/// Read and canonicalize the `<path>` from the command line. If no
/// `<path>` was given, use the current directory.
pub static DOC_PATH: LazyLock<Result<PathBuf, std::io::Error>> = LazyLock::new(|| {
    if let Some(p) = &ARGS.path {
        p.canonicalize()
    } else {
        env::current_dir()
    }
});

/// An enum representing the available verbosity level filters of the logger.
///
/// A `LevelFilter` may be compared directly to a [`Level`]. Use this type
/// to get and set the maximum log level with [`max_level()`] and [`set_max_level`].
///
/// [`Level`]: enum.Level.html
/// [`max_level()`]: fn.max_level.html
/// [`set_max_level`]: fn.set_max_level.html
#[repr(usize)]
#[derive(
    Clone,
    Copy,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Debug,
    Hash,
    Serialize,
    Deserialize,
    Default,
    ValueEnum,
)]
pub enum ClapLevelFilter {
    /// A level lower than all log levels.
    Off,
    /// Corresponds to the `Error` log level.
    #[default]
    Error,
    /// Corresponds to the `Warn` log level.
    Warn,
    /// Corresponds to the `Info` log level.
    Info,
    /// Corresponds to the `Debug` log level.
    Debug,
    /// Corresponds to the `Trace` log level.
    Trace,
}
