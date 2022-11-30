//! Set configuration defaults, reads and writes _Tp-Note_'s configuration file
//! and exposes the configuration as `static` variable.
use crate::settings::ARGS;
use crate::VERSION;
use directories::ProjectDirs;
use lazy_static::lazy_static;
use log::LevelFilter;
#[cfg(not(test))]
use sanitize_filename_reader_friendly::TRIM_LINE_CHARS;
use serde::Deserialize;
use serde::Serialize;
use std::collections::HashMap;
use std::fs;
#[cfg(not(test))]
use std::fs::File;
#[cfg(not(test))]
use std::io::Write;
use std::path::Path;
use std::path::PathBuf;
use std::sync::RwLock;
use tpnote_lib::config::Filename;
use tpnote_lib::config::LocalLinkKind;
use tpnote_lib::config::Tmpl;
use tpnote_lib::config::TmplHtml;
#[cfg(not(test))]
use tpnote_lib::config::FILENAME_DOTFILE_MARKER;
#[cfg(not(test))]
use tpnote_lib::config::LIB_CFG;
use tpnote_lib::error::FileError;
use tpnote_lib::filename::NotePathBuf;

/// Name of this executable (without the Windows ".exe" extension).
const CARGO_BIN_NAME: &str = env!("CARGO_BIN_NAME");

/// Tp-Note's configuration file filename.
const CONFIG_FILENAME: &str = concat!(env!("CARGO_BIN_NAME"), ".toml");

/// Default value for the command line option `--debug`.  Determines the maximum
/// debug level events must have, to be logged.  If the command line option
/// `--debug` is present, its value will be used instead.
const ARG_DEFAULT_DEBUG: LevelFilter = LevelFilter::Error;

/// Default value for the command line flag `--edit` to disable file watcher,
/// (Markdown)-renderer, html server and a web browser launcher set to `true`.
const ARG_DEFAULT_EDITOR: bool = false;

/// Default value for the command line flag `--no-filename-sync` to disable
/// the title to filename synchronisation mechanism permanently.
/// If set to `true`, the corresponding command line flag is ignored.
const ARG_DEFAULT_NO_FILENAME_SYNC: bool = false;

/// Default value for the command line flag `--popup`. If the command line flag
/// `--popup` or `POPUP` is `true`, all log events will also trigger the
/// appearance of a popup alert window.  Note, that error level debug events
/// will always pop up, regardless of `--popup` and `POPUP` (unless
/// `--debug=off`).
const ARG_DEFAULT_POPUP: bool = true;

/// Default value for the command line flag `--tty`. _Tp-Note_ tries different
/// heuristics to detect weather a graphic environment is available or not. For
/// example, under Linux, the '`DISPLAY`' environment variable is evaluated. The
/// '`--tty`' flag disables the automatic detection and sets _Tp-Note_ in
/// "console" mode, where only the non GUI editor (see configuration variable:
/// '`[app_args] editor_console`') and no viewer is launched. If this is set
/// to `true` _Tp-Note_ starts in console mode permanently.
const ARG_DEFAULT_TTY: bool = false;

/// Default value for the command line flag `--add-header`. If unset,
/// _Tp-Note_ exits of when it tries to open a text file without a YAML
/// header. When this flag is set, the missing header is constructed by
/// means of the text file's filename and creation date.
const ARG_DEFAULT_ADD_HEADER: bool = true;

/// By default clipboard support is enabled, can be disabled
/// in config file. A false value here will set ENABLE_EMPTY_CLIPBOARD to
/// false.
const CLIPBOARD_READ_ENABLED: bool = true;

/// Should the clipboard be emptied when tp-note closes?
/// Default value.
const CLIPBOARD_EMPTY_ENABLED: bool = true;

/// Default command line argument list when launching the web browser.
/// The list is executed item by item until an installed web browser is found.
/// Can be changed in config file.
#[cfg(all(target_family = "unix", not(target_vendor = "apple")))]
const APP_ARGS_BROWSER: &[&[&str]] = &[
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
const APP_ARGS_BROWSER: &[&[&str]] = &[
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
const APP_ARGS_BROWSER: &[&[&str]] = &[];

/// Default command line argument list when launching external editor.
/// The editor list is executed item by item until an editor is found.
/// Can be changed in config file.
#[cfg(all(target_family = "unix", not(target_vendor = "apple")))]
const APP_ARGS_EDITOR: &[&[&str]] = &[
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
    // Disable Typora until bug fix:
    // https://github.com/typora/typora-issues/issues/4633
    //    &["typora"],
    &["retext"],
    &["geany", "-s", "-i", "-m"],
    &["gedit", "-w"],
    &["mousepad", "--disable-server"],
    &["leafpad"],
    &["nvim-qt", "--nofork"],
    &["gvim", "--nofork"],
];
#[cfg(target_family = "windows")]
const APP_ARGS_EDITOR: &[&[&str]] = &[
    // Disable Typora until bug fix:
    // https://github.com/typora/typora-issues/issues/4633
    //    &["C:\\Program Files\\Typora\\Typora.exe"],
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
const APP_ARGS_EDITOR: &[&[&str]] = &[
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
const APP_ARGS_EDITOR_CONSOLE: &[&[&str]] =
    &[&["hx"], &["nvim"], &["nano"], &["vim"], &["emacs"], &["vi"]];
#[cfg(target_family = "windows")]
const APP_ARGS_EDITOR_CONSOLE: &[&[&str]] = &[&["hx"], &["nvim"]];
// Some info about launching programs on iOS:
// [dshell.pdf](https://www.stata.com/manuals13/dshell.pdf)
#[cfg(all(target_family = "unix", target_vendor = "apple"))]
const APP_ARGS_EDITOR_CONSOLE: &[&[&str]] = &[
    &["hx"],
    &["nvim"],
    &["nano"],
    &["pico"],
    &["vim"],
    &["emacs"],
    &["vi"],
];

/// When Tp-Note starts, it launches two external applications: some text editor
/// and the viewer (web browser). By default the two programs are launched at
/// the same time (`VIEWER_STARTUP_DELAY==0`). If `VIEWER_STARTUP_DELAY>0` the
/// viewer (web browser) will be launched `VIEWER_STARTUP_DELAY` milliseconds
/// after the text editor. If `VIEWER_STARTUP_DELAY<0` the viewer will be
/// started first. Common values are `-1000`, `0` and `1000`.
const VIEWER_STARTUP_DELAY: isize = 500;

/// When set to true, the viewer feature is automatically disabled when
/// _Tp-Note_ encounters an `.md` file without header.  Experienced users can
/// set this to `true`. This setting is ignored, meaning is considered `false`,
/// if `ARG_DEFAULT_ADD_HEADER=true` or `ARGS.add_header=true` or
/// `ARGS.viewer=true`.
const VIEWER_MISSING_HEADER_DISABLES: bool = false;

/// How often should the file watcher check for changes?
/// Delay in milliseconds. Maximum value is 2000.
const VIEWER_NOTIFY_PERIOD: u64 = 1000;

/// The maximum number of TCP connections the HTTP server can handle at the same
/// time. In general, the serving and live update of the HTML rendition of the
/// note file, requires normally 3 TCP connections: 1 old event channel (that is
/// still open from the previous update), 1 TCP connection to serve the HTML,
/// the local images (and referenced documents), and 1 new event channel.  In
/// practise, stale connection are not always closed immediately. Hence 4 open
/// connections are not uncommon.
const VIEWER_TCP_CONNECTIONS_MAX: usize = 16;

/// Served file types with corresponding mime types.
/// The first entry per line is the file extension in lowercase(!), the second the
/// corresponding mime type.  Embedded files with types other than those listed
/// here are silently ignored.  Note, that image files must be located in the
/// same or in the note's parent directory.
const VIEWER_SERVED_MIME_TYPES: &[&[&str]] = &[
    &["md", "text/x-markdown"],
    &["txt", "text/plain"],
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

/// For security reasons, Tp-Note's internal viewer only displays a limited
/// number number of Tp-Note files when browsing between files.
/// This variable limits this number.
const VIEWER_DISPLAYED_TPNOTE_COUNT_MAX: usize = 10;

/// Configuration data, deserialized from the configuration file.
#[derive(Debug, Serialize, Deserialize)]
pub struct Cfg {
    /// Version number of the config file as String -or-
    /// a text message explaining why we could not load the
    /// configuration file.
    pub version: String,
    pub arg_default: ArgDefault,
    pub filename: Filename,
    pub clipboard: Clipboard,
    pub tmpl: Tmpl,
    pub app_args: AppArgs,
    pub viewer: Viewer,
    pub tmpl_html: TmplHtml,
}

/// Command line arguments, deserialized form configuration file.
#[derive(Debug, Serialize, Deserialize)]
pub struct ArgDefault {
    pub debug: LevelFilter,
    pub edit: bool,
    pub no_filename_sync: bool,
    pub popup: bool,
    pub tty: bool,
    pub add_header: bool,
    pub export_link_rewriting: LocalLinkKind,
}

/// Configuration of clipboard behaviour, deserialized from the
/// configuration file.
#[derive(Debug, Serialize, Deserialize)]
pub struct Clipboard {
    pub read_enabled: bool,
    pub empty_enabled: bool,
}

/// Arguments lists for invoking external applications, deserialized from the
/// configuration file.
#[derive(Debug, Serialize, Deserialize)]
pub struct AppArgs {
    pub browser: Vec<Vec<String>>,
    pub editor: Vec<Vec<String>>,
    pub editor_console: Vec<Vec<String>>,
}

/// Configuration data for the viewer feature, deserialized from the
/// configuration file.
#[derive(Debug, Serialize, Deserialize)]
pub struct Viewer {
    pub startup_delay: isize,
    pub missing_header_disables: bool,
    pub notify_period: u64,
    pub tcp_connections_max: usize,
    pub served_mime_types: Vec<Vec<String>>,
    pub displayed_tpnote_count_max: usize,
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
            arg_default: ArgDefault::default(),
            tmpl: Tmpl::default(),
            app_args: AppArgs::default(),
            clipboard: Clipboard::default(),
            filename: Filename::default(),
            viewer: Viewer::default(),
            tmpl_html: TmplHtml::default(),
        }
    }
}

/// Default values for command line arguments.
impl ::std::default::Default for ArgDefault {
    fn default() -> Self {
        ArgDefault {
            debug: ARG_DEFAULT_DEBUG,
            edit: ARG_DEFAULT_EDITOR,
            no_filename_sync: ARG_DEFAULT_NO_FILENAME_SYNC,
            popup: ARG_DEFAULT_POPUP,
            tty: ARG_DEFAULT_TTY,
            add_header: ARG_DEFAULT_ADD_HEADER,
            export_link_rewriting: LocalLinkKind::Long,
        }
    }
}

/// Default values for invoking external applications.
impl ::std::default::Default for AppArgs {
    fn default() -> Self {
        AppArgs {
            editor: APP_ARGS_EDITOR
                .iter()
                .map(|i| i.iter().map(|a| (*a).to_string()).collect())
                .collect(),
            editor_console: APP_ARGS_EDITOR_CONSOLE
                .iter()
                .map(|i| i.iter().map(|a| (*a).to_string()).collect())
                .collect(),
            browser: APP_ARGS_BROWSER
                .iter()
                .map(|i| i.iter().map(|a| (*a).to_string()).collect())
                .collect(),
        }
    }
}

/// Default values for clipboard behaviour.
impl ::std::default::Default for Clipboard {
    fn default() -> Self {
        Clipboard {
            read_enabled: CLIPBOARD_READ_ENABLED,
            empty_enabled: CLIPBOARD_EMPTY_ENABLED,
        }
    }
}

/// Default values for the viewer feature.
impl ::std::default::Default for Viewer {
    fn default() -> Self {
        Viewer {
            startup_delay: VIEWER_STARTUP_DELAY,
            missing_header_disables: VIEWER_MISSING_HEADER_DISABLES,
            notify_period: VIEWER_NOTIFY_PERIOD,
            tcp_connections_max: VIEWER_TCP_CONNECTIONS_MAX,
            served_mime_types: VIEWER_SERVED_MIME_TYPES
                .iter()
                .map(|i| i.iter().map(|a| (*a).to_string()).collect())
                .collect(),
            displayed_tpnote_count_max: VIEWER_DISPLAYED_TPNOTE_COUNT_MAX,
        }
    }
}

lazy_static! {
    /// Store the extension as key and mime type as value in HashMap.
    pub static ref VIEWER_SERVED_MIME_TYPES_HMAP: HashMap<&'static str, &'static str> = {
        let mut hm = HashMap::new();
        for l in &CFG.viewer.served_mime_types {
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
fn config_load(config_path: &Path) -> Result<Cfg, FileError> {
    if config_path.exists() {
        let config: Cfg = toml::from_str(&fs::read_to_string(config_path)?)?;
        // Check for obvious configuration errors.
        if config
            .filename
            .sort_tag_chars
            .find(config.filename.sort_tag_extra_separator)
            .is_some()
            || config.filename.sort_tag_extra_separator == FILENAME_DOTFILE_MARKER
        {
            return Err(FileError::ConfigFileSortTag {
                char: FILENAME_DOTFILE_MARKER,
                chars: config.filename.sort_tag_chars.escape_default().to_string(),
                extra_separator: config
                    .filename
                    .sort_tag_extra_separator
                    .escape_default()
                    .to_string(),
            });
        }

        // Check for obvious configuration errors.
        if !TRIM_LINE_CHARS.contains(&config.filename.copy_counter_extra_separator) {
            return Err(FileError::ConfigFileCopyCounter {
                chars: TRIM_LINE_CHARS.escape_default().to_string(),
                extra_separator: config
                    .filename
                    .copy_counter_extra_separator
                    .escape_default()
                    .to_string(),
            });
        }
        {
            // Copy the parts of `config` into `LIB_CFG`.
            let mut lib_cfg = LIB_CFG.write().unwrap();
            (*lib_cfg).filename = config.filename.clone();
            (*lib_cfg).tmpl = config.tmpl.clone();
            (*lib_cfg).tmpl_html = config.tmpl_html.clone();
        }

        // First check passed.
        Ok(config)
    } else {
        let cfg = Cfg::default();
        config_write(&cfg, config_path)?;
        Ok(cfg)
    }
}

/// In unit tests we use the default configuration values.
#[cfg(test)]
#[inline]
fn config_load(_config_path: &Path) -> Result<Cfg, FileError> {
    Ok(Cfg::default())
}

/// Writes the default configuration to `Path`.
#[cfg(not(test))]
fn config_write(config: &Cfg, config_path: &Path) -> Result<(), FileError> {
    fs::create_dir_all(config_path.parent().unwrap_or_else(|| Path::new("")))?;

    let mut buffer = File::create(config_path)?;
    buffer.write_all(toml::to_string_pretty(config)?.as_bytes())?;
    Ok(())
}

/// In unit tests we do not write anything.
#[cfg(test)]
fn config_write(_config: &Cfg, _config_path: &Path) -> Result<(), FileError> {
    Ok(())
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

        config_load(config_path)
            .unwrap_or_else(|e|{
                // Remember that something went wrong.
                let mut cfg_file_loading = CFG_FILE_LOADING.write().unwrap();
                *cfg_file_loading = Err(e);

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
            let config = ProjectDirs::from("rs", "", CARGO_BIN_NAME)?;

            let mut config = PathBuf::from(config.config_dir());
            config.push(Path::new(CONFIG_FILENAME));
            Some(config)
        }
    };
}

pub fn backup_config_file() -> Result<PathBuf, FileError> {
    if let Some(ref config_path) = *CONFIG_PATH {
        if config_path.exists() {
            let mut config_path_bak = config_path.clone();
            config_path_bak.set_next_unused()?;

            fs::rename(&config_path.as_path(), &config_path_bak)?;

            config_write(&Cfg::default(), config_path)?;

            Ok(config_path_bak)
        } else {
            Err(FileError::ConfigFileNotFound)
        }
    } else {
        Err(FileError::PathToConfigFileNotFound)
    }
}
