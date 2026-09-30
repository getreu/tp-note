//! Custom error types.
use std::path::PathBuf;
use std::process::ExitStatus;
use thiserror::Error;
use tpnote_lib::error::FileError;
use tpnote_lib::error::LibCfgError;
use tpnote_lib::error::NoteError;

#[allow(dead_code)]
#[derive(Debug, Error)]
/// Error arising in the `workflow` and `main` module.
pub enum WorkflowError {
    /// Remedy: check `<path>` to note file.
    #[error("Can not export. No note file found.")]
    ExportNeedsNoteFile,

    /// Remedy: restart with `--debug trace`.
    #[error(
        "Failed to render template (cf. `{tmpl_name}`\
         in configuration file)!\n{source}"
    )]
    Template {
        tmpl_name: String,
        source: NoteError,
    },

    #[error(transparent)]
    Note(#[from] NoteError),

    #[error(transparent)]
    ConfigFile(#[from] ConfigFileError),

    #[error(transparent)]
    Io(#[from] std::io::Error),

    #[error(transparent)]
    IoRef(#[from] &'static std::io::Error),
}

/// Error related to the filesystem and to invoking external applications.
#[derive(Debug, Error)]
pub enum ConfigFileError {
    /// Remedy: Compare your config file structure with the default one
    /// (`--config-defaults`).
    #[error(
        "Unknown top level key(s) in configuration file:\n\
        {error:?}"
    )]
    ConfigFileUnkownFieldName { error: Vec<String> },

    /// Remedy: check the path and permissions of the to be generated
    /// configuration file.
    #[error(
        "Can not write the default configuration:\n\
        ---\n\
        {error}"
    )]
    ConfigFileWrite { error: String },

    /// Remedy: restart, or check file permission of the configuration file.
    #[error(
        "Can not load or parse the (merged)\n\
        configuration file(s):\n\
        ---\n\
        {error}\n\n\
        Note: this error may occur after upgrading\n\
        Tp-Note due to some incompatible configuration\n\
        file changes.\n\
        \n\
        Tp-Note continues with its internal default\n\
        configuration."
    )]
    ConfigFileLoadParse { error: String },

    /// Remedy: fix or remove the offending file. Unlike
    /// `ConfigFileLoadParse`, this is not fatal: the file is skipped and
    /// Tp-Note continues with the rest of the merged configuration (cf.
    /// the CUSTOMIZATION section of the man page).
    #[error(
        "Ignoring configuration file:\n\
        ---\n\
        {}\n\n\
        {error}",
        path.display()
    )]
    ConfigFileSkipped { path: PathBuf, error: String },

    /// Remedy: move the editor/browser launch commands you want into your
    /// user configuration file instead (cf. the CUSTOMIZATION section of
    /// the man page). Unlike `ConfigFileSkipped`, the rest of this file's
    /// settings were still applied; only `[app_args]` was dropped.
    #[error(
        "Ignoring `[app_args]` in project configuration file:\n\
        ---\n\
        {}\n\n\
        A project configuration file (found by searching upward from the\n\
        note's directory) is not allowed to set `app_args.*.editor`,\n\
        `app_args.*.editor_console` or `app_args.*.browser`: such a file can\n\
        live in a directory you do not control (a cloned repository, an\n\
        extracted archive, a synced folder), and these settings launch an\n\
        arbitrary external program. Set them in your user or system\n\
        configuration file instead. The rest of this file's settings were\n\
        applied normally.",
        path.display()
    )]
    ConfigFileAppArgsIgnored { path: PathBuf },

    /// Remedy: restart.
    #[error(
        "Configuration file version mismatch:\n---\n\
        Configuration file version: \'{config_file_version}\'\n\
        Minimum required version: \'{min_version}\'\n\
        \n\
        Tp-Note continues, using this configuration file as is."
    )]
    ConfigFileVersionMismatch {
        config_file_version: String,
        min_version: String,
    },

    /// Should not happen. Please report this bug.
    #[error("Can not convert path to UTF-8:\n{path:?}")]
    PathNotUtf8 { path: PathBuf },

    /// Remedy: check the configuration file variable `app_args.editor`.
    #[error(
        "The external application did not terminate\n\
         gracefully: {code}\n\
         \n\
         Edit the variable `{var_name}` in Tp-Note's\n\
         configuration file and correct the following:\n\
         \t{args:?}"
    )]
    ApplicationReturn {
        code: ExitStatus,
        var_name: String,
        args: Vec<String>,
    },

    /// Remedy: check the configuration file variable `app_args.editor`
    /// or `app_args.browser` depending on the displayed variable name.
    /// For `TPNOTE_EDITOR` and `TPNOTE_BROWSER` check the environment
    /// variable of the same name.
    #[error(
        "Can not find any external application listed\n\
        in `{var_name}`: \
        {app_list}\n\
        Install one of the listed applications on your\n\
        system -or- register some already installed\n\
        application in Tp-Note's configuration file\n\
        or in the corresponding environment variable."
    )]
    NoApplicationFound { app_list: String, var_name: String },

    #[error(transparent)]
    File(#[from] FileError),

    #[error(transparent)]
    LibConfig(#[from] LibCfgError),

    #[error(transparent)]
    Io(#[from] std::io::Error),

    #[error(transparent)]
    IoRef(#[from] &'static std::io::Error),

    #[error(transparent)]
    Serialize(#[from] toml::ser::Error),

    #[error(transparent)]
    Deserialize(#[from] toml::de::Error),
}

/// One entry per configuration file that was skipped while merging multiple
/// sources (always the `ConfigFileSkipped` variant, never fatal), as
/// opposed to a bare `ConfigFileError` returned from a `Result`, which is.
pub type ConfigFileWarnings = Vec<ConfigFileError>;
