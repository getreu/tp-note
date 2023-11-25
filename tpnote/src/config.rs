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
use std::fs;
use std::fs::File;
use std::io;
use std::mem;
use std::path::Path;
use std::path::PathBuf;
use tera::Tera;
use tpnote_lib::config::LocalLinkKind;
use tpnote_lib::config::Scheme;
use tpnote_lib::config::TmplHtml;
use tpnote_lib::config::FILENAME_ROOT_PATH_MARKER;
use tpnote_lib::config::LIB_CFG;
use tpnote_lib::config::LIB_CONFIG_DEFAULT_TOML;
use tpnote_lib::context::Context;
use tpnote_lib::filename::NotePathBuf;

/// Set the minimum required configuration file version that is compatible with
/// this Tp-Note version.
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
/// 3. Disable minimum version check; all configuration file versions are
///    allowed.
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

/// This decides until what depth arrays are merged into the default
/// configuration. Tables are always merged. Deeper arrays replace the default
/// configuration. For our configuration this means, that `scheme` is merged and
/// all other arrays are replaced.
pub(crate) const CONFIG_FILE_MERGE_DEPTH: isize = 2;

/// Configuration data, deserialized from the configuration file.
#[derive(Debug, Serialize, Deserialize)]
pub struct Cfg {
    /// Version number of the configuration file as String -or- a text message
    /// explaining why we could not load the configuration file.
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
    pub scheme: String,
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
            scheme: "default".to_string(),
            tty: false,
            add_header: true,
            export_link_rewriting: LocalLinkKind::default(),
        }
    }
}

/// Configuration of clipboard behaviour, deserialized from the configuration
/// file.
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
    #[inline]
    fn from_files(config_paths: &[PathBuf]) -> Result<Cfg, ConfigFileError> {
        /// `CONFIG_FILE_MERGE_DEPTH` controls whether a top-level array in
        /// the TOML document is merged instead of overridden. This is useful
        /// for TOML documents that use a top-level array of values like the
        /// `tpnote.toml`, where one usually wants to override or add to the
        /// array instead of replacing it altogether.
        fn merge_toml_values(
            left: toml::Value,
            right: toml::Value,
            merge_depth: isize,
        ) -> toml::Value {
            use toml::Value;

            fn get_name(v: &Value) -> Option<&str> {
                v.get("name").and_then(Value::as_str)
            }

            match (left, right) {
                (Value::Array(mut left_items), Value::Array(right_items)) => {
                    // The top-level arrays should be merged but nested arrays
                    // should act as overrides. For the `tpnote.toml` config,
                    // this means that you can specify a sub-set of schemes in
                    // an overriding `tpnote.toml` but that nested arrays like
                    // `schme.tmpl.fm_var_localization` are replaced instead
                    // of merged.
                    if merge_depth > 0 {
                        left_items.reserve(right_items.len());
                        for rvalue in right_items {
                            let lvalue = get_name(&rvalue)
                                .and_then(|rname| {
                                    left_items.iter().position(|v| get_name(v) == Some(rname))
                                })
                                .map(|lpos| left_items.remove(lpos));
                            let mvalue = match lvalue {
                                Some(lvalue) => merge_toml_values(lvalue, rvalue, merge_depth - 1),
                                None => rvalue,
                            };
                            left_items.push(mvalue);
                        }
                        Value::Array(left_items)
                    } else {
                        Value::Array(right_items)
                    }
                }
                (Value::Table(mut left_map), Value::Table(right_map)) => {
                    if merge_depth > -10 {
                        for (rname, rvalue) in right_map {
                            match left_map.remove(&rname) {
                                Some(lvalue) => {
                                    let merged_value =
                                        merge_toml_values(lvalue, rvalue, merge_depth - 1);
                                    left_map.insert(rname, merged_value);
                                }
                                None => {
                                    left_map.insert(rname, rvalue);
                                }
                            }
                        }
                        Value::Table(left_map)
                    } else {
                        Value::Table(right_map)
                    }
                }
                (_, value) => value,
            }
        }

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

        //
        // `from_files()` start
        let base_config = toml::from_str(&Self::default_as_toml())?;

        let config = config_paths
            .iter()
            .filter_map(|file| {
                std::fs::read_to_string(file)
                    .map(|config| toml::from_str(&config))
                    .ok()
            })
            .collect::<Result<Vec<_>, _>>()?
            .into_iter()
            .fold(base_config, |a, b| {
                merge_toml_values(a, b, CONFIG_FILE_MERGE_DEPTH)
            });

        // We can not use the logger here, it is too early.
        if ARGS.debug == Some(LevelFilter::Trace) && ARGS.batch && ARGS.version {
            eprintln!(
                "*** Merged configuration from all config files:\n\n{:#?}",
                config
            );
        }

        let mut config: Cfg = config.try_into()?;

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

        // We can not use the logger here, it is too early.
        if ARGS.debug == Some(LevelFilter::Trace) && ARGS.batch && ARGS.version {
            eprintln!(
                "\n\n\n\n\n*** Configuration after applied templates:\n\n\
                {:#?}\
                \n\n\n\n\n",
                config
            );
        }

        // First check passed.
        Ok(config)
    }

    /// Writes the default configuration to `Path` or to `stdout` if
    /// `config_path == -`.
    pub(crate) fn write_default_to_file_or_stdout(
        config_path: &Path,
    ) -> Result<(), ConfigFileError> {
        // These must live longer than `readable`, and thus are declared first:
        let (mut stdout_write, mut file_write);
        // On-Stack Dynamic Dispatch:
        let writeable: &mut dyn io::Write = if config_path == Path::new("-") {
            stdout_write = io::stdout();
            &mut stdout_write
        } else {
            fs::create_dir_all(config_path.parent().unwrap_or_else(|| Path::new("")))?;
            file_write = File::create(config_path)?;
            &mut file_write
        };
        writeable.write_all(Self::default_as_toml().as_bytes())?;
        Ok(())
    }

    /// Backs up the existing configuration file and writes a new one with
    /// default values.
    pub(crate) fn backup_and_remove_last() -> Result<PathBuf, ConfigFileError> {
        if let Some(config_path) = CONFIG_PATHS.iter().filter(|p| p.exists()).last() {
            let mut config_path_bak = config_path.to_path_buf();
            config_path_bak.set_next_unused()?;
            fs::rename(config_path, &config_path_bak)?;

            Ok(config_path.clone())
        } else {
            Err(ConfigFileError::PathToConfigFileNotFound)
        }
    }
}

lazy_static! {
    /// Reads and parses the configuration file "tp-note.toml". An alternative
    /// filename (optionally with absolute path) can be given on the command
    /// line with "--config".
    pub static ref CFG: Cfg = {
            Cfg::from_files(&CONFIG_PATHS)
                .unwrap_or_else(|e|{
                    // Remember that something went wrong.
                    let mut cfg_file_loading = CFG_FILE_LOADING.write();
                    *cfg_file_loading = Err(e);

                    // As we could not load the configuration file, we will use
                    // the default configuration.
                    Cfg::default()
                })
        };
}

lazy_static! {
    /// Variable indicating with `Err` if the loading of the configuration file
    /// went wrong.
    pub static ref CFG_FILE_LOADING: RwLock<Result<(), ConfigFileError>> = RwLock::new(Ok(()));
}

lazy_static! {
/// This is where the Tp-Note searches for its configuration files.
    pub static ref CONFIG_PATHS : Vec<PathBuf> = {

        let mut config_path: Vec<PathBuf> = vec![];

        #[cfg(unix)]
        config_path.push(PathBuf::from("/etc/tpnote/tpnote.toml"));


        // Config path comes from the environment variable.
            if let Ok(env_config) = env::var(ENV_VAR_TPNOTE_CONFIG) {
               config_path.push(PathBuf::from(env_config));
            };
        // Config comes from the standard configuration file location.
        if let Some(usr_config) = ProjectDirs::from("rs", "", CARGO_BIN_NAME){

            let mut config = PathBuf::from(usr_config.config_dir());
            config.push(Path::new(CONFIG_FILENAME));
            config_path.push(config);
        };

        // Is there a `FILENAME_ROOT_PATH_MARKER` file?
        if let Some(root_path) = DOC_PATH.as_deref().ok()
            .map(|doc_path| {
                        let mut root_path = Context::from(doc_path).root_path;
                        root_path.push(FILENAME_ROOT_PATH_MARKER);
                        root_path
            }){
            config_path.push(root_path);
        };

        if let Some(commandline_path) = &ARGS.config {
                // Config path comes from command line.
                config_path.push(PathBuf::from(commandline_path));
        };

        config_path
    };
}

#[cfg(test)]
mod tests {
    use super::Cfg;
    use std::env::temp_dir;
    use std::fs;

    #[test]
    fn test_cfg_from_file() {
        // Prepare test: some mini config file.
        let raw = "\
        [arg_default]
        scheme = 'zettel'
        ";
        let userconfig = temp_dir().join("tpnote.toml");
        fs::write(&userconfig, raw.as_bytes()).unwrap();

        let cfg = Cfg::from_files(&[userconfig]).unwrap();
        assert_eq!(cfg.arg_default.scheme, "zettel");

        // Prepare test: create existing note.
        let raw = "\
        [viewer]
        served_mime_types = [ ['abc', 'abc/text'], ]
        ";
        let userconfig = temp_dir().join("tpnote.toml");
        fs::write(&userconfig, raw.as_bytes()).unwrap();

        let cfg = Cfg::from_files(&[userconfig]).unwrap();
        assert_eq!(cfg.viewer.served_mime_types.len(), 1);
        assert_eq!(cfg.viewer.served_mime_types[0].0, "abc");

        // Prepare test: create existing note.
        let raw = "\
        [[scheme]]
        name = 'default'
        [scheme.filename]
        sort_tag.separator = '---'
        [scheme.tmpl]
        fm_var.localization = [ ['fm_foo', 'foofoo'], ]
        ";
        let userconfig = temp_dir().join("tpnote.toml");
        fs::write(&userconfig, raw.as_bytes()).unwrap();

        let cfg = Cfg::from_files(&[userconfig]).unwrap();
        assert_eq!(cfg.scheme.len(), 2);
        assert_eq!(cfg.scheme[0].name, "zettel");
        assert_eq!(cfg.scheme[0].filename.sort_tag.separator, "--");
        assert_eq!(cfg.scheme[1].name, "default");
        assert_eq!(cfg.scheme[1].filename.sort_tag.separator, "---");
        assert_eq!(cfg.scheme[1].tmpl.fm_var.localization.len(), 1);
        assert_eq!(
            cfg.scheme[1].tmpl.fm_var.localization[0],
            ("fm_foo".to_string(), "foofoo".to_string())
        );

        // Prepare test: merge (replace) the default config into itself.
        let raw = Cfg::default_as_toml();
        let userconfig = temp_dir().join("tpnote.toml");
        fs::write(&userconfig, raw.as_bytes()).unwrap();

        let cfg = Cfg::from_files(&[userconfig]).unwrap();
        // `len=2` means that `scheme.default` is replaced by `scheme.default`
        // and `scheme.zettel` is replaced `scheme.zettel`.
        assert_eq!(cfg.scheme.len(), 2);
        assert_eq!(cfg.scheme[0].name, "default");
        assert_eq!(cfg.scheme[1].name, "zettel");
    }
}
