//! Sets configuration defaults, reads, and writes Tp-Note's configuration
//! file and exposes the configuration as `static` variable.
use crate::error::ConfigFileError;
use crate::error::ConfigFileWarnings;
use crate::settings::ARGS;
use crate::settings::ClapLevelFilter;
use crate::settings::DOC_PATH;
use crate::settings::ENV_VAR_TPNOTE_CONFIG;
use directories::ProjectDirs;
use parking_lot::RwLock;
use serde::Serialize;
use serde::{Deserialize, Deserializer};
use std::collections::HashMap;
use std::env;
use std::fs;
use std::fs::File;
use std::io;
use std::mem;
use std::path::Path;
use std::path::PathBuf;
use std::str::FromStr;
use std::sync::LazyLock;
use tera::Tera;
use toml::Value;
use tpnote_lib::config::EmbeddedContentErrorPolicy;
use tpnote_lib::config::LIB_CFG;
use tpnote_lib::config::LIB_CFG_RAW_FIELD_NAMES;
use tpnote_lib::config::LIB_CONFIG_DEFAULT_TOML;
use tpnote_lib::config::LibCfg;
use tpnote_lib::config::LocalLinkKind;
use tpnote_lib::config::TmplHtml;
use tpnote_lib::config_value::CfgVal;
use tpnote_lib::text_reader::read_as_string_with_crlf_suppression;

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

/// Name of this executable (without the Windows `.exe` extension).
pub(crate) const CARGO_BIN_NAME: &str = env!("CARGO_BIN_NAME");

/// Use the version number defined in `../Cargo.toml`.
pub(crate) const PKG_VERSION: Option<&'static str> = option_env!("CARGO_PKG_VERSION");

/// Tp-Note's configuration file filename.
const CONFIG_FILENAME: &str = concat!(env!("CARGO_BIN_NAME"), ".toml");

/// Default configuration.
pub(crate) const GUI_CONFIG_DEFAULT_TOML: &str = include_str!("config_default.toml");

pub(crate) const DO_NOT_COMMENT_IF_LINE_STARTS_WITH: [&str; 3] = ["###", "[", "name ="];

/// Configuration data, deserialized from the configuration file.
#[derive(Debug, Serialize, Deserialize)]
pub struct Cfg {
    /// Only meaningful in a directory marker file found while searching
    /// upward for the document root (cf. the CUSTOMIZATION section of the
    /// man page). Ignored everywhere else.
    #[serde(default)]
    pub project_config: ProjectConfig,
    /// Version number of the configuration file as String -or- a text message
    /// explaining why we could not load the configuration file.
    pub version: String,
    #[serde(flatten)]
    pub extra_fields: HashMap<String, Value>,
    pub arg_default: ArgDefault,
    pub clipboard: Clipboard,
    pub app_args: OsType<AppArgs>,
    pub viewer: Viewer,
    pub tmpl_html: TmplHtml,
}

/// Directives a directory marker file (`tpnote.toml` found while searching
/// upward for the document root) can set under `[project_config]`. Ignored
/// everywhere else (cf. the CUSTOMIZATION section of the man page).
#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProjectConfig {
    #[serde(default = "default_true")]
    pub is_root_path_marker: bool,
    #[serde(default)]
    pub merge_parent_config: bool,
}

impl Default for ProjectConfig {
    fn default() -> Self {
        Self {
            is_root_path_marker: true,
            merge_parent_config: false,
        }
    }
}

fn default_true() -> bool {
    true
}

#[derive(Debug, Serialize, Deserialize, Default)]
#[serde(deny_unknown_fields)]
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
#[serde(deny_unknown_fields)]
pub struct ArgDefault {
    pub debug: ClapLevelFilter,
    pub edit: bool,
    #[serde(deserialize_with = "deserialize_empty_string_as_none")]
    pub force_lang: Option<String>,
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
            debug: ClapLevelFilter::Error,
            edit: false,
            force_lang: None,
            no_filename_sync: false,
            popup: false,
            scheme: "default".to_string(),
            tty: false,
            add_header: true,
            export_link_rewriting: LocalLinkKind::default(),
        }
    }
}

/// Configuration of clipboard behavior, deserialized from the configuration
/// file.
#[derive(Debug, Serialize, Deserialize, Default)]
#[serde(deny_unknown_fields)]
pub struct Clipboard {
    pub read_enabled: bool,
    pub empty_enabled: bool,
}

/// Arguments lists for invoking external applications, deserialized from the
/// configuration file.
#[derive(Debug, Serialize, Deserialize, Default)]
#[serde(deny_unknown_fields)]
pub struct AppArgs {
    pub browser: Vec<Vec<String>>,
    pub editor: Vec<Vec<String>>,
    pub editor_console: Vec<Vec<String>>,
}

/// How the viewer restricts access to the OS user running Tp-Note
/// (`viewer.same_user_policy`). On a multi-user machine the loopback port is
/// reachable by every logged-in user; this check rejects connections whose
/// owning process belongs to a different OS user. It is best-effort
/// defense-in-depth (the connection→user lookup can be inconclusive), so the
/// policy decides whether to enforce at all.
/// Deserialized from a PascalCase TOML string (`"Off"`/`"Enforce"`),
/// mirroring `LocalLinkKind`.
#[cfg(feature = "same-user-policy")]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum SameUserPolicy {
    /// No user check — serve any local process (previous behaviour).
    Off,
    /// Serve only a peer proven to be the same OS user; refuse everything else
    /// (fail-closed). A peer proven to belong to a *different* user **and** a
    /// peer whose user cannot be determined (sandbox / network namespace,
    /// lookup race, platform privilege limits — the common case for a foreign
    /// user on Linux, where a non-root viewer cannot resolve another user's
    /// process) are both refused. This is the default and the safest posture.
    /// The cost is that a legitimate client whose user cannot be resolved (a
    /// sandboxed browser) is refused; such a user is shown a page explaining
    /// how to disable the check (`Off`).
    #[default]
    Enforce,
}

/// Configuration data for the viewer feature, deserialized from the
/// configuration file.
///
/// CAUTION: for `session_binding_cookie` the derived `Default` (`false`,
/// protection off) does not match the shipped `config_default.toml` (`true`);
/// `same_user_policy` derives its `#[default]` variant `Enforce`, matching the
/// shipped default. `CFG` is always built from `config_default.toml`, not
/// `Viewer::default()`, but keep this in mind before calling
/// `Viewer::default()` elsewhere.
#[derive(Debug, Serialize, Deserialize, Default)]
#[serde(deny_unknown_fields)]
pub struct Viewer {
    pub startup_delay: isize,
    pub missing_header_disables: bool,
    pub notify_period: u64,
    pub tcp_connections_max: usize,
    pub served_mime_types: Vec<(String, String)>,
    pub displayed_tpnote_count_max: usize,
    pub session_binding_cookie: bool,
    #[cfg(feature = "same-user-policy")]
    pub same_user_policy: SameUserPolicy,
}

/// When no configuration file is found, defaults are set here from built-in
/// constants. These defaults are then serialized into a newly created
/// configuration file on disk.
impl ::std::default::Default for Cfg {
    fn default() -> Self {
        // Make sure that we parse the `LIB_CONFIG_DEFAULT_TOML` first.
        LazyLock::force(&LIB_CFG);

        toml::from_str(&Cfg::default_as_toml()).expect(
            "Error in default configuration in source file:\n\
                 `tpnote/src/config_default.toml`",
        )
    }
}

impl Cfg {
    /// Emits the default configuration as TOML string with comments.
    ///
    /// `[project_config]` is appended last, after `LIB_CONFIG_DEFAULT_TOML`
    /// and `GUI_CONFIG_DEFAULT_TOML`: TOML has no syntax to "close" a table
    /// and return to the root, so any table header placed before those
    /// constants would swallow their leading root-level scalars (e.g.
    /// `LIB_CONFIG_DEFAULT_TOML` starts with the root-level
    /// `scheme_sync_default`) into that table instead. Putting
    /// `[project_config]` last avoids that trap regardless of what either
    /// constant starts with.
    #[inline]
    fn default_as_toml() -> String {
        let config_default_toml = format!(
            "version = \"{}\"\n\n\
             {}\n\n{}\n\n\
             ### Only meaningful in a directory marker file found while searching\n\
             ### upward for the document root (cf. the CUSTOMIZATION section of\n\
             ### the man page). Ignored everywhere else.\n\
             [project_config]\n\n\
             ### Whether this marker file fixes the document root here.\n\
             is_root_path_marker = true\n\n\
             ### Whether to keep searching further up for additional parent\n\
             ### configuration to merge underneath this file.\n\
             merge_parent_config = false\n",
            PKG_VERSION.unwrap_or_default(),
            LIB_CONFIG_DEFAULT_TOML,
            GUI_CONFIG_DEFAULT_TOML
        );

        config_default_toml
    }

    /// Checks whether `cfg_val` deserializes into a valid, fully specified
    /// configuration, without mutating any global state (in particular,
    /// without touching `LIB_CFG`). Used to decide, one file at a time,
    /// whether merging a candidate configuration layer keeps the
    /// accumulated result usable.
    fn validate(cfg_val: &CfgVal) -> Result<(), ConfigFileError> {
        LibCfg::try_from(cfg_val.clone())?;
        let cfg: Cfg = cfg_val.clone().to_value().try_into()?;
        let unused: Vec<String> = cfg
            .extra_fields
            .into_keys()
            .filter(|k| !LIB_CFG_RAW_FIELD_NAMES.contains(&k.as_str()))
            .collect();
        if !unused.is_empty() {
            return Err(ConfigFileError::ConfigFileUnkownFieldName { error: unused });
        }
        Ok(())
    }

    /// Parse the configuration file if it exists. Otherwise write one with
    /// default values. The second element of the returned tuple holds one
    /// message per configuration file that had to be skipped (cf. the
    /// comment on `CFG_FILE_WARNINGS`).
    #[inline]
    fn from_files(config_paths: &[PathBuf]) -> Result<(Cfg, ConfigFileWarnings), ConfigFileError> {
        // Runs through all strings and renders config values as templates.
        // No variables are set in this context. But you can use environment
        // variables in templates: e.g.:
        //    `{{ get_env(name="username", default="unknown-user" )}}`.
        fn render_tmpl(var: &mut [Vec<String>]) {
            var.iter_mut().for_each(|i| {
                i.iter_mut().for_each(|arg| {
                    let new_arg = Tera::default()
                        .render_str(arg, &tera::Context::new(), false)
                        .unwrap_or_default()
                        .to_string();
                    let _ = mem::replace(arg, new_arg);
                })
            })
        }

        //
        // `from_files()` start
        let mut base_config = CfgVal::from_str(GUI_CONFIG_DEFAULT_TOML)?;
        base_config.extend(CfgVal::from_str(LIB_CONFIG_DEFAULT_TOML)?);
        base_config.insert(
            "version".to_string(),
            Value::String(PKG_VERSION.unwrap_or_default().to_string()),
        );

        // Merge all config files from various locations. Each file is
        // applied one at a time and only kept if the result still parses
        // into a valid configuration; a broken file (invalid TOML, wrong
        // types, unknown keys) is skipped with a warning instead of
        // discarding every other successfully loaded layer. This matters
        // most for the directory marker chain (`merge_parent_config`),
        // where the number of files merged can grow with the depth of the
        // search.
        let mut cfg_val = base_config;
        let mut warnings: ConfigFileWarnings = Vec::new();
        for path in config_paths {
            let Ok(reader) = File::open(path) else {
                continue;
            };
            let parsed = read_as_string_with_crlf_suppression(reader)
                .map_err(ConfigFileError::from)
                .and_then(|config| {
                    toml::from_str::<CfgVal>(&config).map_err(ConfigFileError::from)
                });

            let file_val = match parsed {
                Ok(v) => v,
                Err(e) => {
                    warnings.push(ConfigFileError::ConfigFileSkipped {
                        path: path.clone(),
                        error: e.to_string(),
                    });
                    continue;
                }
            };

            let candidate = cfg_val.clone().merge(file_val);
            match Self::validate(&candidate) {
                Ok(()) => cfg_val = candidate,
                Err(e) => warnings.push(ConfigFileError::ConfigFileSkipped {
                    path: path.clone(),
                    error: format!("The merged configuration would be invalid:\n{e}"),
                }),
            }
        }
        // We cannot he logger here, it is too early.
        if ARGS.debug == Some(ClapLevelFilter::Trace) && ARGS.batch && ARGS.version {
            println!(
                "*** Merged configuration from all config files:\n\n{:#?}",
                cfg_val
            );
        }

        // Parse Values into the `lib_cfg`.
        let lib_cfg = LibCfg::try_from(cfg_val.clone())?;
        {
            // Copy the `lib_cfg` into `LIB_CFG`.
            let mut c = LIB_CFG.write();
            *c = lib_cfg; // Release lock.

            // In batch export (`-x` together with `-b`) a broken embedded
            // renderer (Mermaid diagram or LaTeX formula) must fail the pipeline
            // rather than emit an error box into a published file. Force the
            // exporter policy to `HardError`, overriding the configured value.
            if ARGS.export.is_some() && ARGS.batch {
                c.tmpl_html.exporter_embedded_content_error_policy =
                    EmbeddedContentErrorPolicy::HardError;
            }

            // We cannot use the logger here, it is too early.
            if ARGS.debug == Some(ClapLevelFilter::Trace) && ARGS.batch && ARGS.version {
                println!(
                    "\n\n\n\n\n*** Configuration part 1 after merging \
                    `scheme`s into copies of `base_scheme`:\
                    \n\n{:#?}\
                    \n\n\n\n\n",
                    *c
                );
            }
        } // Release lock.

        //
        // Parse the result into the struct `Cfg`.
        let mut cfg: Cfg = cfg_val.to_value().try_into()?;

        // Collect unused field names.
        // We know that all keys collected in `extra_fields` must be
        // top level keys in `LIB_CFG`.
        let unused: Vec<String> = cfg
            .extra_fields
            .into_keys()
            .filter(|k| !LIB_CFG_RAW_FIELD_NAMES.contains(&k.as_str()))
            .collect::<Vec<String>>();
        if !unused.is_empty() {
            return Err(ConfigFileError::ConfigFileUnkownFieldName { error: unused });
        };

        // Remove already processed items.
        cfg.extra_fields = HashMap::new();

        // Fill in potential templates.
        render_tmpl(&mut cfg.app_args.unix.browser);
        render_tmpl(&mut cfg.app_args.unix.editor);
        render_tmpl(&mut cfg.app_args.unix.editor_console);

        render_tmpl(&mut cfg.app_args.windows.browser);
        render_tmpl(&mut cfg.app_args.windows.editor);
        render_tmpl(&mut cfg.app_args.windows.editor_console);

        render_tmpl(&mut cfg.app_args.macos.browser);
        render_tmpl(&mut cfg.app_args.macos.editor);
        render_tmpl(&mut cfg.app_args.macos.editor_console);

        let cfg = cfg; // Freeze.

        // We cannot use the logger here, it is too early.
        if ARGS.debug == Some(ClapLevelFilter::Trace) && ARGS.batch && ARGS.version {
            println!(
                "\n\n\n\n\n*** Configuration part 2 after applied templates:\
                \n\n{:#?}\
                \n\n\n\n\n",
                cfg
            );
        }
        // First check passed.
        Ok((cfg, warnings))
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

        let mut commented = String::new();
        for l in Self::default_as_toml().lines() {
            if l.is_empty() {
                commented.push('\n');
            } else if DO_NOT_COMMENT_IF_LINE_STARTS_WITH
                .iter()
                .all(|&token| !l.starts_with(token))
            {
                commented.push_str("# ");
                commented.push_str(l);
                commented.push('\n');
            } else {
                commented.push_str(l);
                commented.push('\n');
            }
        }
        writeable.write_all(commented.as_bytes())?;
        Ok(())
    }
}

/// Reads and parses the configuration file "tpnote.toml". An alternative
/// filename (optionally with absolute path) can be given on the command
/// line with "--config".
pub static CFG: LazyLock<Cfg> = LazyLock::new(|| {
    let (cfg, warnings) = Cfg::from_files(&PROJECT_PATHS.config_paths).unwrap_or_else(|e| {
        // Remember that something went wrong.
        let mut cfg_file_loading = CFG_FILE_LOADING.write();
        *cfg_file_loading = Err(e);

        // As we could not load the configuration file, we will use
        // the default configuration.
        (Cfg::default(), Vec::new())
    });
    *CFG_FILE_WARNINGS.write() = warnings;
    cfg
});

/// Variable indicating with `Err` if the loading of the configuration file
/// went wrong.
pub static CFG_FILE_LOADING: LazyLock<RwLock<Result<(), ConfigFileError>>> =
    LazyLock::new(|| RwLock::new(Ok(())));

/// One message per configuration file that `Cfg::from_files()` skipped
/// because it was invalid on its own, or made the merged result invalid.
/// Populated too early to be logged directly (cf. the comment in
/// `Cfg::from_files()`); `main()` logs these with `log::warn!()` once the
/// logger's level filter is in its final state.
pub static CFG_FILE_WARNINGS: LazyLock<RwLock<ConfigFileWarnings>> =
    LazyLock::new(|| RwLock::new(Vec::new()));

/// The appearance of a file with this filename marks the position of the
/// document root (cf. `ProjectPaths::walk_project_paths()`).
const FILENAME_ROOT_PATH_MARKER: &str = "tpnote.toml";

/// The deserialization view of a `tpnote.toml` marker file limited to its
/// `[project_config]` table (cf. the `ProjectConfig` struct above); every
/// other key in the file is irrelevant here.
#[derive(Debug, Deserialize, Default)]
struct ProjectConfigFile {
    #[serde(default)]
    project_config: ProjectConfig,
}

/// The document root together with the two path lists derived from the
/// single upward directory-marker search for `DOC_PATH`'s directory. The
/// climb (and the reading and parsing of every candidate's
/// `[project_config]` table) only ever runs once per process; `ROOT_PATH`
/// and every other consumer of the config/searched path lists reads from
/// this one static. If `DOC_PATH` is unavailable, `root_path` is empty and
/// both path lists collapse to just the fixed candidates and the command
/// line override; `workflow::run()` fails on that same condition before any
/// of them is ever consulted.
pub(crate) struct ProjectPaths {
    /// The document root: the directory where the upward search for a
    /// `tpnote.toml` marker file stopped (cf. the CUSTOMIZATION section of
    /// the man page).
    pub(crate) root_path: PathBuf,
    /// Every configuration file that actually gets merged: the fixed
    /// per-platform candidates, then every marker that was actually found
    /// walking up from the note's directory (farthest first, closest last,
    /// so the closest one takes precedence when merged), then the command
    /// line override.
    pub(crate) config_paths: Vec<PathBuf>,
    /// Like `config_paths`, but the directory-marker portion additionally
    /// lists every ancestor directory the upward search considered,
    /// whether or not it held a `tpnote.toml` file. Used only for
    /// `--version`'s `searched_config_file_paths`; never for merging,
    /// since most of these entries do not exist on disk.
    pub(crate) searched_paths: Vec<PathBuf>,
}

impl ProjectPaths {
    /// Walks upward from `dir_path` collecting every ancestor directory that
    /// contains a `FILENAME_ROOT_PATH_MARKER` file, then decides, marker by
    /// marker starting from the closest, where the document root lies and how
    /// far the search for additional configuration extends.
    ///
    /// Returns `(root_path, config_chain, searched_chain)`, both `config_chain`
    /// and `searched_chain` ordered farthest first, closest last, ready to be
    /// merged with lower-precedence layers applied first.
    ///
    /// * `root_path` is fixed at the first marker (closest to farthest) whose
    ///   `project_config.is_root_path_marker` is `true` or absent -- the
    ///   default, chosen for every marker file written before this option
    ///   existed. If no marker declares itself the root, `root_path` falls
    ///   back to the filesystem root, matching the behavior when no marker
    ///   file exists at all.
    /// * The search for additional configuration files continues past a given
    ///   marker only if that marker's own `project_config.merge_parent_config`
    ///   is `true`. The first marker (again, closest to farthest) that leaves
    ///   it at the default `false` ends the search; `root_path` is unaffected
    ///   by how far this search extends.
    /// * `config_chain` only lists the markers that were actually found (and
    ///   are thus merged into the configuration); `searched_chain` lists every
    ///   candidate path this search considered, including ancestor directories
    ///   that turned out to have no marker file. `searched_chain` is only ever
    ///   assembled into `ProjectPaths::searched_paths` for reporting (cf.
    ///   `--version`'s `searched_config_file_paths`) -- it must never be used
    ///   for merging, since most of its entries do not exist.
    fn walk_project_paths(dir_path: &Path) -> (PathBuf, Vec<PathBuf>, Vec<PathBuf>) {
        let mut fallback_root = dir_path;
        let mut root_path: Option<PathBuf> = None;
        let mut config_chain: Vec<PathBuf> = Vec::new();
        let mut searched_chain: Vec<PathBuf> = Vec::new();

        for anc in dir_path.ancestors() {
            fallback_root = anc;
            let marker = anc.join(FILENAME_ROOT_PATH_MARKER);
            searched_chain.push(marker.clone());
            if !marker.is_file() {
                continue;
            }

            // A conscious choice: if this file can't be read or parsed, fall
            // back to the defaults (root marker, do not extend the search)
            // instead of treating it as absent. Excluding it here would let the
            // search continue past it, silently widening the document root --
            // and thus the viewer's security boundary -- past a directory the
            // user never asked to expose, just because of a typo. The file is
            // still handed on to the full configuration merge below, which
            // parses it independently and reports it via `ConfigFileWarnings`
            // if it is indeed broken -- so the failure is surfaced, not hidden,
            // without ever widening the boundary to compensate for it.
            let flags = fs::read_to_string(&marker)
                .ok()
                .and_then(|s| toml::from_str::<ProjectConfigFile>(&s).ok())
                .unwrap_or_default()
                .project_config;

            config_chain.push(marker.clone());

            if root_path.is_none() && flags.is_root_path_marker {
                root_path = marker.parent().map(Path::to_path_buf);
            }

            if !flags.merge_parent_config {
                break;
            }
        }

        let root_path = root_path.unwrap_or_else(|| fallback_root.to_owned());

        // Farthest first, closest last: closest overrides on merge.
        config_chain.reverse();
        searched_chain.reverse();

        (root_path, config_chain, searched_chain)
    }

    /// Assembles the fixed per-platform candidate locations and the command
    /// line override around `marker_chain`, the directory-marker portion of the
    /// path list. Called twice while building `ProjectPaths`: once for the
    /// markers that were actually found (`ProjectPaths::config_paths`, used for
    /// merging), once for every candidate the upward search considered
    /// (`ProjectPaths::searched_paths`, used only for reporting).
    fn assemble_config_paths(marker_chain: &[PathBuf]) -> Vec<PathBuf> {
        let mut config_path: Vec<PathBuf> = vec![];

        #[cfg(unix)]
        config_path.push(PathBuf::from("/etc/tpnote/tpnote.toml"));

        // The user's configuration file. Its location can be overridden with
        // the environment variable, in which case the standard per-platform
        // location below is not consulted.
        if let Ok(env_config) = env::var(ENV_VAR_TPNOTE_CONFIG) {
            config_path.push(PathBuf::from(env_config));
        } else if let Some(usr_config) = ProjectDirs::from("rs", "", CARGO_BIN_NAME) {
            let mut config = PathBuf::from(usr_config.config_dir());
            config.push(Path::new(CONFIG_FILENAME));
            config_path.push(config);
        };

        config_path.extend_from_slice(marker_chain);

        if let Some(commandline_path) = &ARGS.config {
            // Config path comes from command line.
            config_path.push(PathBuf::from(commandline_path));
        };

        config_path
    }

    /// Builds `PROJECT_PATHS`: runs the upward directory-marker search for
    /// `DOC_PATH`'s directory (if `DOC_PATH` is available) and assembles the
    /// resulting root path and the two config-path lists around it.
    fn new() -> Self {
        let Ok(doc_path) = DOC_PATH.as_deref() else {
            return ProjectPaths {
                root_path: PathBuf::new(),
                config_paths: ProjectPaths::assemble_config_paths(&[]),
                searched_paths: ProjectPaths::assemble_config_paths(&[]),
            };
        };
        let dir_path = if doc_path.is_dir() {
            doc_path.to_path_buf()
        } else {
            doc_path
                .parent()
                .unwrap_or_else(|| Path::new("./"))
                .to_path_buf()
        };
        let (root_path, config_chain, searched_chain) =
            ProjectPaths::walk_project_paths(&dir_path);
        ProjectPaths {
            root_path,
            config_paths: ProjectPaths::assemble_config_paths(&config_chain),
            searched_paths: ProjectPaths::assemble_config_paths(&searched_chain),
        }
    }
}

pub(crate) static PROJECT_PATHS: LazyLock<ProjectPaths> = LazyLock::new(ProjectPaths::new);

/// The document root: the directory where the upward search for a
/// `tpnote.toml` marker file stopped (cf. the CUSTOMIZATION section of the
/// man page). Passed into `WorkflowBuilder::new()` and `Context::from()` so
/// they do not need to repeat the search `PROJECT_PATHS` already performed
/// for the same directory.
pub static ROOT_PATH: LazyLock<PathBuf> = LazyLock::new(|| PROJECT_PATHS.root_path.clone());

fn deserialize_empty_string_as_none<'de, D>(deserializer: D) -> Result<Option<String>, D::Error>
where
    D: Deserializer<'de>,
{
    let s = String::deserialize(deserializer)?;
    if s.is_empty() { Ok(None) } else { Ok(Some(s)) }
}

#[cfg(test)]
mod tests {
    use tpnote_lib::config::LIB_CFG;

    use super::Cfg;
    #[cfg(feature = "same-user-policy")]
    use super::SameUserPolicy;
    use std::env::temp_dir;
    use std::fs;

    #[test]
    fn test_cfg_from_file() {
        //
        // Prepare test: some mini config file.
        let raw = "\
        [arg_default]
        scheme = 'zettel'
        ";
        let userconfig = temp_dir().join("tpnote.toml");
        fs::write(&userconfig, raw.as_bytes()).unwrap();

        let (cfg, _warnings) = Cfg::from_files(&[userconfig]).unwrap();
        assert_eq!(cfg.arg_default.scheme, "zettel");
        // A user config lacking the key inherits the built-in default `true`.
        assert!(cfg.viewer.session_binding_cookie);

        //
        // Prepare test: create existing note.
        let raw = "\
        [viewer]
        served_mime_types = [ ['abc', 'abc/text'], ]
        ";
        let userconfig = temp_dir().join("tpnote.toml");
        fs::write(&userconfig, raw.as_bytes()).unwrap();

        let (cfg, _warnings) = Cfg::from_files(&[userconfig]).unwrap();
        assert_eq!(cfg.viewer.served_mime_types.len(), 1);
        assert_eq!(cfg.viewer.served_mime_types[0].0, "abc");

        //
        // Prepare test: the session binding can be disabled.
        let raw = "\
        [viewer]
        session_binding_cookie = false
        ";
        let userconfig = temp_dir().join("tpnote.toml");
        fs::write(&userconfig, raw.as_bytes()).unwrap();

        let (cfg, _warnings) = Cfg::from_files(&[userconfig]).unwrap();
        assert!(!cfg.viewer.session_binding_cookie);

        //
        // Prepare test: `same_user_policy` defaults to `Enforce` and parses.
        #[cfg(feature = "same-user-policy")]
        {
            let userconfig = temp_dir().join("tpnote.toml");
            fs::write(&userconfig, b"").unwrap();
            let (cfg, _warnings) = Cfg::from_files(&[userconfig]).unwrap();
            assert_eq!(cfg.viewer.same_user_policy, SameUserPolicy::Enforce);

            let raw = "\
            [viewer]
            same_user_policy = \"Off\"
            ";
            let userconfig = temp_dir().join("tpnote.toml");
            fs::write(&userconfig, raw.as_bytes()).unwrap();
            let (cfg, _warnings) = Cfg::from_files(&[userconfig]).unwrap();
            assert_eq!(cfg.viewer.same_user_policy, SameUserPolicy::Off);

            // The dropped `Warn` value no longer aborts the whole load: the
            // offending file is skipped with a warning and the built-in
            // default is used instead.
            let raw = "\
            [viewer]
            same_user_policy = \"Warn\"
            ";
            let userconfig = temp_dir().join("tpnote.toml");
            fs::write(&userconfig, raw.as_bytes()).unwrap();
            let (cfg, _warnings) = Cfg::from_files(&[userconfig]).unwrap();
            assert_eq!(cfg.viewer.same_user_policy, SameUserPolicy::Enforce);

            // Likewise for an invalid enum string.
            let raw = "\
            [viewer]
            same_user_policy = \"bogus\"
            ";
            let userconfig = temp_dir().join("tpnote.toml");
            fs::write(&userconfig, raw.as_bytes()).unwrap();
            let (cfg, _warnings) = Cfg::from_files(&[userconfig]).unwrap();
            assert_eq!(cfg.viewer.same_user_policy, SameUserPolicy::Enforce);
        }

        //
        // Prepare test: an unknown top level key no longer aborts the whole
        // load either; the file is skipped with a warning.
        let raw = "\
        unknown_field_name = 'aha'
        ";
        let userconfig = temp_dir().join("tpnote.toml");
        fs::write(&userconfig, raw.as_bytes()).unwrap();

        // No longer an error: the file is skipped, defaults apply.
        let (_cfg, warnings) = Cfg::from_files(&[userconfig]).unwrap();
        assert_eq!(warnings.len(), 1);

        //
        // Prepare test: one bad file among several does not discard the
        // others -- the good ones still take effect. The skip is not
        // silent: it is reported back in the returned warning list, which
        // `main()` logs once the logger's filter allows it through (cf.
        // `CFG_FILE_WARNINGS`).
        let good_config = temp_dir().join("tpnote-good.toml");
        fs::write(&good_config, "[arg_default]\nscheme = 'zettel'\n").unwrap();
        let bad_config = temp_dir().join("tpnote-bad.toml");
        fs::write(&bad_config, "unknown_field_name = 'aha'\n").unwrap();

        let (cfg, warnings) = Cfg::from_files(&[good_config, bad_config.clone()]).unwrap();
        assert_eq!(cfg.arg_default.scheme, "zettel");
        assert_eq!(warnings.len(), 1);
        assert!(
            warnings[0]
                .to_string()
                .contains(&bad_config.display().to_string())
        );

        //
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

        let (_cfg, _warnings) = Cfg::from_files(&[userconfig]).unwrap();
        {
            let lib_cfg = LIB_CFG.read();
            // The variables come from the `./config_default.toml` `zettel`
            // scheme:
            assert_eq!(lib_cfg.scheme.len(), 2);
            let zidx = lib_cfg.scheme_idx("zettel").unwrap();
            assert_eq!(lib_cfg.scheme[zidx].name, "zettel");
            // This variable is defined in the `zettel` scheme in
            // `./config_default.toml`:
            assert_eq!(lib_cfg.scheme[zidx].filename.sort_tag.separator, "--");
            // This variables are inherited from the `base_scheme` in
            // `./config_default.toml`. They are part of the `zettel` scheme:
            assert_eq!(lib_cfg.scheme[zidx].filename.extension_default, "md");
            assert_eq!(lib_cfg.scheme[zidx].filename.sort_tag.extra_separator, '\'');

            let didx = lib_cfg.scheme_idx("default").unwrap();
            // This variables are inherited from the `base_scheme` in
            // `./config_default.toml`. They are part of the `default` scheme:
            assert_eq!(lib_cfg.scheme[didx].filename.extension_default, "md");
            assert_eq!(lib_cfg.scheme[didx].tmpl.fm_var.localization.len(), 1);
            // These variables originate from `userconfig` (`tpnote.toml`)
            // and are part of the `default` scheme:
            assert_eq!(lib_cfg.scheme[didx].filename.sort_tag.separator, "---");
            assert_eq!(
                lib_cfg.scheme[didx].tmpl.fm_var.localization[0],
                ("fm_foo".to_string(), "foofoo".to_string())
            );
            // This variable is defined in the `default` scheme in
            // `./config_default.toml`:
            assert_eq!(lib_cfg.scheme[didx].name, "default");
        } // Free `LIB_CFG` lock.
    }
}
