//! Reads the command line parameters and clipboard and exposes them as `static`
//! variables.

//#[cfg(any(feature = "read-clipboard", feature = "viewer"))]
use crate::clipboard::SystemClipboard;
use crate::config::CFG;
use log::LevelFilter;
use std::env;
use std::io;
use std::io::IsTerminal;
use std::io::Read;
use std::path::PathBuf;
use std::sync::LazyLock;
use structopt::StructOpt;
use tpnote_lib::config::LocalLinkKind;
use tpnote_lib::content::Content;
use tpnote_lib::content::ContentString;

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

#[derive(Debug, Eq, PartialEq, StructOpt)]
#[structopt(
    name = "Tp-Note",
    about = "Fast note taking with templates and filename synchronization."
)]
/// _Tp-Note_ is a note-taking tool and a template system, that synchronizes the
/// note's metadata with its filename. _Tp-Note_ collects various information
/// about its environment and the clipboard and stores it in variables. New
/// notes are created by filling these variables in predefined and customizable
/// `Tera`-templates. In case `<path>` points to an existing _Tp-Note_ file, the
/// note's metadata is analyzed and, if necessary, its filename is adjusted.
/// For all other file types, _Tp-Note_ creates a new note annotating the
/// file `<path>` points to. If `<path>` is a directory (or, when omitted the
/// current working directory), a new note is created in that directory. After
/// creation, _Tp-Note_ launches an external editor of your choice. Although the
/// templates are written for Markdown, _Tp-Note_ is not tied to
/// any specific markup language. However, _Tp-Note_ comes with an optional
/// viewer feature, that currently renders only Markdown, ReStructuredText and
/// HTML. Note, that there is also some limited support for Asciidoc and
/// WikiText. The note's rendition with its hyperlinks is live updated and
/// displayed in the user's webbrowser.
pub struct Args {
    /// Prepends YAML header if missing
    #[structopt(long, short = "a")]
    pub add_header: bool,
    /// Batch mode: does not launch editor or viewer
    #[structopt(long, short = "b")]
    pub batch: bool,
    /// Loads (and merges) an additional configuration file
    #[structopt(long, short = "c")]
    pub config: Option<String>,
    /// Dumps the internal default configuration into a file
    /// or stdout if `-`
    #[structopt(long, short = "C")]
    pub config_defaults: Option<String>,
    /// Console debug level: "trace", "debug", "info", "warn", "error"
    /// (default) or "off"
    #[structopt(long, short = "d")]
    pub debug: Option<LevelFilter>,
    /// Shows console debug messages also as popup windows
    #[structopt(long, short = "u")]
    pub popup: bool,
    /// Launches only the editor, no browser
    #[structopt(long, short = "e")]
    pub edit: bool,
    /// Scheme for new notes: "default", "zettel", (cf. `--config-defaults`)
    #[structopt(long, short = "s")]
    pub scheme: Option<String>,
    /// Forces console mode: opens console editor, no browser
    #[structopt(long, short = "t")]
    pub tty: bool,
    /// Lets web server listen to a specific port
    #[structopt(long, short = "p")]
    pub port: Option<u16>,
    /// Disables filename synchronization
    #[structopt(long, short = "n")]
    pub no_filename_sync: bool,
    /// Disables the automatic language detection and uses `<force-lang>`
    /// instead; or, if '' use `TPNOTE_LANG` or `LANG`
    #[structopt(long, short = "l")]
    pub force_lang: Option<String>,
    /// Launches only the browser, no editor
    #[structopt(long, short = "v")]
    pub view: bool,
    /// `<dir>` the new note's location or `<file>` to open or to annotate
    #[structopt(name = "PATH", parse(from_os_str))]
    pub path: Option<PathBuf>,
    /// Prints version and exits
    #[structopt(long, short = "V")]
    pub version: bool,
    /// Saves the HTML rendition in the `<export>` directory,
    /// the note's directory if '' or stdout if '-'.
    #[structopt(long, short = "x", parse(from_os_str))]
    pub export: Option<PathBuf>,
    /// Exporter local link rewriting: "off", "short", "long" (default)
    #[structopt(long)]
    pub export_link_rewriting: Option<LocalLinkKind>,
}

/// Structure to hold the parsed command line arguments.
pub static ARGS: LazyLock<Args> = LazyLock::new(Args::from_args);

/// Shall we launch the external text editor?
pub static LAUNCH_EDITOR: LazyLock<bool> = LazyLock::new(|| {
    !ARGS.batch
        && ARGS.export.is_none()
        && env::var(ENV_VAR_TPNOTE_EDITOR) != Ok(String::new())
        && (ARGS.edit || !ARGS.view)
});

#[cfg(feature = "viewer")]
/// Shall we launch the internal http server and the external browser?
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
/// Shall we launch the internal http server and the external browser?
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

/// Reads the input stream stdin if there is any.
pub static STDIN: LazyLock<ContentString> = LazyLock::new(|| {
    let mut buffer = String::new();

    // Read stdin().
    let stdin = io::stdin();
    if !stdin.is_terminal() {
        // There is an input pipe for us to read from.
        let mut handle = stdin.lock();
        let _ = handle.read_to_string(&mut buffer);
    }

    ContentString::from_string_with_cr(buffer)
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
