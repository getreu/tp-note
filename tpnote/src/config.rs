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
use std::collections::BTreeMap;
use std::env;
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
use tpnote_lib::config::Filename;
use tpnote_lib::config::LocalLinkKind;
use tpnote_lib::config::Tmpl;
use tpnote_lib::config::TmplHtml;
use tpnote_lib::config::FILENAME_ROOT_PATH_MARKER;
use tpnote_lib::config::LIB_CFG;
use tpnote_lib::config::LIB_CONFIG_DEFAULT_TOML;
use tpnote_lib::context::Context;
use tpnote_lib::filename::NotePathBuf;

/// Set the minimum required config file version that is compatible with this
/// Tp-Note version.
///
/// Examples how to use this constant. Choose one of the following:
/// 1. Require some minimum version of the config file.
///    Abort if not satisfied.
///
///    ```no_run
///    const MIN_CONFIG_FILE_VERSION: Option<&'static str> = Some("1.5.1");
///    ```
///
/// 2. Require the config file to be of the same version as this binary.
///
///    ```no_run
///    const MIN_CONFIG_FILE_VERSION: Option<&'static str> = PKG_VERSION;
///    ```
///
/// 3. Disable minimum version check; all config file versions are allowed.
///
///    ```no_run
///    const MIN_CONFIG_FILE_VERSION: Option<&'static str> = None;
///    ```
///
pub(crate) const MIN_CONFIG_FILE_VERSION: Option<&'static str> = Some("1.22.1");

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

/// Default configuragtion.
pub(crate) const GUI_CONFIG_DEFAULT_TOML: &str = include_str!("config_default.toml");

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

/// Configuration data, subset of `Cfg`.
#[derive(Debug, Serialize, Deserialize, Default)]
pub struct GuiCfg {
    /// Version number of the config file as String -or-
    /// a text message explaining why we could not load the
    /// configuration file.
    pub arg_default: ArgDefault,
    pub clipboard: Clipboard,
    pub app_args_unix: AppArgs,
    pub app_args_windows: AppArgs,
    pub app_args_mac: AppArgs,
    pub viewer: Viewer,
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
    pub served_mime_types: Vec<Vec<String>>,
    pub displayed_tpnote_count_max: usize,
}

/// When no configuration file is found, defaults are set here from built-in
/// constants. These defaults are then serialized into a newly created
/// configuration file on disk.
impl ::std::default::Default for Cfg {
    fn default() -> Self {
        #[cfg(not(target_family = "windows"))]
        let config_default_toml = format!(
            "version = \"{}\"\n\n{}\n\n{}",
            PKG_VERSION.unwrap_or_default(),
            GUI_CONFIG_DEFAULT_TOML,
            LIB_CONFIG_DEFAULT_TOML
        );

        #[cfg(target_family = "windows")]
        let config_default_toml = format!(
            "version = \"{}\"\r\n\r\n{}\r\n\r\n{}",
            PKG_VERSION.unwrap_or_default(),
            GUI_CONFIG_DEFAULT_TOML,
            LIB_CONFIG_DEFAULT_TOML
        );

        // Make sure that we parse the `LIB_CONFIG_DEFAULT_TOML` first.
        lazy_static::initialize(&LIB_CFG);

        let cfg = toml::from_str(&config_default_toml).expect(
            "Error in default configuration in source file:\n\
                 `tpnote/src/config_default.toml`",
        );

        cfg
    }
}

lazy_static! {
    /// Store the extension as key and mime type as value in Map.
    pub static ref VIEWER_SERVED_MIME_TYPES_MAP: BTreeMap<&'static str, &'static str> = {
        let mut hm = BTreeMap::new();
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
    pub static ref CFG_FILE_LOADING: RwLock<Result<(), ConfigFileError>> = RwLock::new(Ok(()));
}

/// Parse the configuration file if it exists. Otherwise write one with default values.
#[cfg(not(test))]
#[inline]
fn config_load(config_path: &Path) -> Result<Cfg, ConfigFileError> {
    if config_path.exists() {
        let mut config: Cfg = toml::from_str(&fs::read_to_string(config_path)?)?;

        // Fill `config.app_args.*` templates with empty `tera::Context`.
        // The latter allows to use environment variables in templates: e.g.:
        // `{{ get_env(name="username", default="unknown-user" )}}`.
        let mut tera = Tera::default();
        let context = tera::Context::new();
        config.app_args.browser.iter_mut().for_each(|i| {
            i.iter_mut().for_each(|arg| {
                let new_arg = tera
                    .render_str(arg, &context)
                    .unwrap_or_default()
                    .to_string();
                let _ = mem::replace(arg, new_arg);
            })
        });
        config.app_args.editor.iter_mut().for_each(|i| {
            i.iter_mut().for_each(|arg| {
                let new_arg = tera
                    .render_str(arg, &context)
                    .unwrap_or_default()
                    .to_string();
                let _ = mem::replace(arg, new_arg);
            })
        });
        config.app_args.editor_console.iter_mut().for_each(|i| {
            i.iter_mut().for_each(|arg| {
                let new_arg = tera
                    .render_str(arg, &context)
                    .unwrap_or_default()
                    .to_string();
                let _ = mem::replace(arg, new_arg);
            })
        });
        let config = config; // Freeze.

        {
            // Copy the parts of `config` into `LIB_CFG`.
            let mut lib_cfg = LIB_CFG.write();
            lib_cfg.filename = config.filename.clone();
            lib_cfg.tmpl = config.tmpl.clone();
            lib_cfg.tmpl_html = config.tmpl_html.clone();

            // Perform some additional semantic checks.
            lib_cfg.assert_validity()?;
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
fn config_load(_config_path: &Path) -> Result<Cfg, ConfigFileError> {
    Ok(Cfg::default())
}

/// Writes the default configuration to `Path`.
#[cfg(not(test))]
fn config_write(config: &Cfg, config_path: &Path) -> Result<(), ConfigFileError> {
    fs::create_dir_all(config_path.parent().unwrap_or_else(|| Path::new("")))?;

    let mut buffer = File::create(config_path)?;
    buffer.write_all(toml::to_string_pretty(config)?.as_bytes())?;
    Ok(())
}

/// In unit tests we do not write anything.
#[cfg(test)]
fn config_write(_config: &Cfg, _config_path: &Path) -> Result<(), ConfigFileError> {
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
                    let mut cfg_file_loading = CFG_FILE_LOADING.write();
                    *cfg_file_loading = Err(ConfigFileError::PathToConfigFileNotFound);
                    return Cfg::default();
                },
            }
        };

        config_load(config_path)
            .unwrap_or_else(|e|{
                // Remember that something went wrong.
                let mut cfg_file_loading = CFG_FILE_LOADING.write();
                *cfg_file_loading = Err(e);

                // As we could not load the config file, we will use the default
                // configuration.
                Cfg::default()
            })
        };
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
                // Config comes from the standard config file location.
                let config = ProjectDirs::from("rs", "", CARGO_BIN_NAME)?;

                let mut config = PathBuf::from(config.config_dir());
                config.push(Path::new(CONFIG_FILENAME));
                Some(config)
            }
        )
    };
}

pub fn backup_config_file() -> Result<PathBuf, ConfigFileError> {
    if let Some(ref config_path) = *CONFIG_PATH {
        if config_path.exists() {
            let mut config_path_bak = config_path.clone();
            config_path_bak.set_next_unused()?;

            fs::rename(config_path.as_path(), &config_path_bak)?;

            config_write(&Cfg::default(), config_path)?;

            Ok(config_path_bak)
        } else {
            Err(ConfigFileError::ConfigFileNotFound)
        }
    } else {
        Err(ConfigFileError::PathToConfigFileNotFound)
    }
}
