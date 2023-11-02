//! Set configuration defaults, reads and writes _Tp-Note_'s configuration file
//! and exposes the configuration as `static` variable.
use crate::error::ConfigFileError;
use crate::settings::ARGS;
use crate::settings::DOC_PATH;
use crate::settings::ENV_VAR_TPNOTE_CONFIG;
use directories::ProjectDirs;
use lazy_static::lazy_static;
use log::LevelFilter;
use parking_lot::RwLock;
use serde::Deserialize;
use serde::Serialize;
use std::env;
#[cfg(not(test))]
use std::fs;
#[cfg(not(test))]
use std::fs::File;
#[cfg(not(test))]
use std::io::Write;
#[cfg(not(test))]
use std::mem;
use std::path::Path;
use std::path::PathBuf;
#[cfg(not(test))]
use tera::Tera;
use tpnote_lib::config::LocalLinkKind;
use tpnote_lib::config::Scheme;
use tpnote_lib::config::TmplHtml;
use tpnote_lib::config::FILENAME_ROOT_PATH_MARKER;
use tpnote_lib::config::LIB_CFG;
use tpnote_lib::config::LIB_CONFIG_DEFAULT_TOML;
use tpnote_lib::context::Context;
#[cfg(not(test))]
use tpnote_lib::filename::NotePathBuf;

/// Set the minimum required configuration file version that is compatible with this
/// Tp-Note version.
///
/// Examples how to use this constant. Choose one of the following:
/// 1. Require some minimum version of the configuration file.
///    Abort if not satisfied.
///
///    ```no_run
///    const MIN_CONFIG_FILE_VERSION: Option<&'static str> = Some("1.5.1");
///    ```
///
/// 2. Require the configuration file to be of the same version as this binary.
///
///    ```no_run
///    const MIN_CONFIG_FILE_VERSION: Option<&'static str> = PKG_VERSION;
///    ```
///
/// 3. Disable minimum version check; all configuration file versions are allowed.
///
///    ```no_run
///    const MIN_CONFIG_FILE_VERSION: Option<&'static str> = None;
///    ```
///
pub(crate) const MIN_CONFIG_FILE_VERSION: Option<&'static str> = PKG_VERSION;

/// Authors.
pub(crate) const AUTHOR: Option<&str> = option_env!("CARGO_PKG_AUTHORS");

/// Copyright.
pub(crate) const COPYRIGHT_FROM: &str = "2020";

/// Name of this executable (without the Windows ".exe" extension).
const CARGO_BIN_NAME: &str = env!("CARGO_BIN_NAME");

/// Use the version number defined in `../Cargo.toml`.
pub(crate) const PKG_VERSION: Option<&'static str> = option_env!("CARGO_PKG_VERSION");

/// Tp-Note's configuration file filename.
const CONFIG_FILENAME: &str = concat!(env!("CARGO_BIN_NAME"), ".toml");

/// Default configuration.
pub(crate) const GUI_CONFIG_DEFAULT_TOML: &str = include_str!("config_default.toml");

/// Configuration data, deserialized from the configuration file.
#[derive(Debug, Serialize, Deserialize)]
pub struct Cfg {
    /// Version number of the configuration file as String -or-
    /// a text message explaining why we could not load the
    /// configuration file.
    pub version: String,
    pub scheme_sync_default: String,
    pub scheme: Vec<Scheme>,
    pub arg_default: ArgDefault,
    pub clipboard: Clipboard,
    pub app_args: OsType<AppArgs>,
    pub viewer: Viewer,
    pub tmpl_html: TmplHtml,
}

#[derive(Debug, Serialize, Deserialize, Default)]
/// The `OsType` selects operating system specific defaults at runtime.
pub struct OsType<T> {
    /// `#[cfg(all(target_family = "unix", not(target_os = "macos")))]`
    /// Currently this selects the following target operating systems:
    /// aix, android, dragonfly, emscripten, espidf, freebsd, fuchsia, haiku,
    /// horizon, illumos, ios, l4re, linux, netbsd, nto, openbsd, redox,
    /// solaris, tvos, unknown, vita, vxworks, wasi, watchos.
    pub unix: T,
    /// `#[cfg(target_family = "windows")]`
    pub windows: T,
    /// `#[cfg(all(target_family = "unix", target_os = "macos"))]`
    pub macos: T,
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

/// Default values for command line arguments.
impl ::std::default::Default for ArgDefault {
    fn default() -> Self {
        ArgDefault {
            debug: LevelFilter::Error,
            edit: false,
            no_filename_sync: false,
            popup: false,
            tty: false,
            add_header: true,
            export_link_rewriting: LocalLinkKind::default(),
        }
    }
}

/// Configuration of clipboard behaviour, deserialized from the
/// configuration file.
#[derive(Debug, Serialize, Deserialize, Default)]
pub struct Clipboard {
    pub read_enabled: bool,
    pub empty_enabled: bool,
}

/// Arguments lists for invoking external applications, deserialized from the
/// configuration file.
#[derive(Debug, Serialize, Deserialize, Default)]
pub struct AppArgs {
    pub browser: Vec<Vec<String>>,
    pub editor: Vec<Vec<String>>,
    pub editor_console: Vec<Vec<String>>,
}

/// Configuration data for the viewer feature, deserialized from the
/// configuration file.
#[derive(Debug, Serialize, Deserialize, Default)]
pub struct Viewer {
    pub startup_delay: isize,
    pub missing_header_disables: bool,
    pub notify_period: u64,
    pub tcp_connections_max: usize,
    pub served_mime_types: Vec<(String, String)>,
    pub displayed_tpnote_count_max: usize,
}

/// When no configuration file is found, defaults are set here from built-in
/// constants. These defaults are then serialized into a newly created
/// configuration file on disk.
impl ::std::default::Default for Cfg {
    fn default() -> Self {
        // Make sure that we parse the `LIB_CONFIG_DEFAULT_TOML` first.
        lazy_static::initialize(&LIB_CFG);

        toml::from_str(&Cfg::default_as_toml()).expect(
            "Error in default configuration in source file:\n\
                 `tpnote/src/config_default.toml`",
        )
    }
}

impl Cfg {
    /// Emits the default configuration as TOML string with comments.
    #[inline]
    fn default_as_toml() -> String {
        #[cfg(not(target_family = "windows"))]
        let config_default_toml = format!(
            "version = \"{}\"\n\n{}\n\n{}",
            PKG_VERSION.unwrap_or_default(),
            LIB_CONFIG_DEFAULT_TOML,
            GUI_CONFIG_DEFAULT_TOML
        );

        #[cfg(target_family = "windows")]
        let config_default_toml = format!(
            "version = \"{}\"\r\n\r\n{}\r\n\r\n{}",
            PKG_VERSION.unwrap_or_default(),
            LIB_CONFIG_DEFAULT_TOML,
            GUI_CONFIG_DEFAULT_TOML
        );

        config_default_toml
    }

    /// Parse the configuration file if it exists. Otherwise write one with
    /// default values.
    #[cfg(not(test))]
    #[inline]
    fn from_file(config_path: &Path) -> Result<Cfg, ConfigFileError> {
        // Runs through all strings and renders config values as templates.
        // No variables are set in this context. But you can use environment
        // variables in templates: e.g.:
        //    `{{ get_env(name="username", default="unknown-user" )}}`.
        fn render_tmpl(var: &mut [Vec<String>]) {
            var.iter_mut().for_each(|i| {
                i.iter_mut().for_each(|arg| {
                    let new_arg = Tera::default()
                        .render_str(arg, &tera::Context::new())
                        .unwrap_or_default()
                        .to_string();
                    let _ = mem::replace(arg, new_arg);
                })
            })
        }

        if config_path.exists() {
            let mut config: Cfg = toml::from_str(&fs::read_to_string(config_path)?)?;

            render_tmpl(&mut config.app_args.unix.browser);
            render_tmpl(&mut config.app_args.unix.editor);
            render_tmpl(&mut config.app_args.unix.editor_console);

            render_tmpl(&mut config.app_args.windows.browser);
            render_tmpl(&mut config.app_args.windows.editor);
            render_tmpl(&mut config.app_args.windows.editor_console);

            render_tmpl(&mut config.app_args.macos.browser);
            render_tmpl(&mut config.app_args.macos.editor);
            render_tmpl(&mut config.app_args.macos.editor_console);

            let config = config; // Freeze.

            {
                // Copy the parts of `config` into `LIB_CFG`.
                let mut lib_cfg = LIB_CFG.write();
                lib_cfg.scheme_sync_default = config.scheme_sync_default.clone();
                lib_cfg.scheme = config.scheme.clone();

                // Perform some additional semantic checks.
                lib_cfg.assert_validity()?;
            }

            // First check passed.
            Ok(config)
        } else {
            Self::write_default_to_file(config_path)?;
            Ok(Self::default())
        }
    }

    /// In unit tests we use the default configuration values.
    #[cfg(test)]
    #[inline]
    fn from_file(_config_path: &Path) -> Result<Cfg, ConfigFileError> {
        Ok(Self::default())
    }

    /// Writes the default configuration to `Path`. If destination exists,
    /// backup it.
    #[cfg(not(test))]
    fn write_default_to_file(config_path: &Path) -> Result<(), ConfigFileError> {
        fs::create_dir_all(config_path.parent().unwrap_or_else(|| Path::new("")))?;

        if config_path.exists() {
            let mut config_path_bak = config_path.to_path_buf();
            config_path_bak.set_next_unused()?;

            fs::rename(config_path, &config_path_bak)?;
        }

        let mut buffer = File::create(config_path)?;
        buffer.write_all(Self::default_as_toml().as_bytes())?;
        Ok(())
    }

    /// In unit tests we do not write anything.
    #[cfg(test)]
    fn write_default_to_file(_config_path: &Path) -> Result<(), ConfigFileError> {
        Ok(())
    }

    /// Backs up the existing configuration file and writes a new one with default
    /// values.
    pub fn backup_and_replace_with_default() -> Result<PathBuf, ConfigFileError> {
        if let Some(ref config_path) = *CONFIG_PATH {
            Self::write_default_to_file(config_path)?;

            Ok(config_path.clone())
        } else {
            Err(ConfigFileError::PathToConfigFileNotFound)
        }
    }
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
                    let mut cfg_file_loading = CFG_FILE_LOADING.write();
                    *cfg_file_loading = Err(ConfigFileError::PathToConfigFileNotFound);
                    return Cfg::default();
                },
            }
        };

        Cfg::from_file(config_path)
            .unwrap_or_else(|e|{
                // Remember that something went wrong.
                let mut cfg_file_loading = CFG_FILE_LOADING.write();
                *cfg_file_loading = Err(e);

                // As we could not load the configuration file, we will use the default
                // configuration.
                Cfg::default()
            })
        };
}

lazy_static! {
    /// Variable indicating with `Err` if the loading of the configuration file went wrong.
    pub static ref CFG_FILE_LOADING: RwLock<Result<(), ConfigFileError>> = RwLock::new(Ok(()));
}

lazy_static! {
/// This is where the Tp-Note searches for its configuration file.
    pub static ref CONFIG_PATH : Option<PathBuf> = {
        use std::fs::File;
        let config_path = if let Some(c) = &ARGS.config {
                // Config path comes from command line.
                Some(PathBuf::from(c))
        } else {
            // Is there a `FILENAME_ROOT_PATH_MARKER` file?
            let root_path = DOC_PATH.as_deref().ok()
                .map(|doc_path| {
                            let mut root_path = Context::from(doc_path).root_path;
                            root_path.push(FILENAME_ROOT_PATH_MARKER);
                            root_path
                });
            // Is this file empty?
            root_path.as_ref()
                .and_then(|root_path| File::open(root_path).ok())
                .and_then(|file| file.metadata().ok())
                .and_then(|metadata|
                    if metadata.len() == 0 {
                        // `FILENAME_ROOT_PATH_MARKER` is empty.
                        None
                    } else {
                        // `FILENAME_ROOT_PATH_MARKER` contains config data.
                        root_path
                    })
        };

        config_path.or_else(||
            // Config path comes from the environment variable.
            if let Ok(c) = env::var(ENV_VAR_TPNOTE_CONFIG) {
               Some(PathBuf::from(c))
            } else {
                // Config comes from the standard configuration file location.
                let config = ProjectDirs::from("rs", "", CARGO_BIN_NAME)?;

                let mut config = PathBuf::from(config.config_dir());
                config.push(Path::new(CONFIG_FILENAME));
                Some(config)
            }
        )
    };
}
