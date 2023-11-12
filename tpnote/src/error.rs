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
    /// Remedy: delete or rename the configuration file.
    #[error(
        "Can not backup and delete the erroneous\n\
        configuration file:\n\
        ---\n\
        {error}\n\n\
        Please do it manually."
    )]
    ConfigFileBackup { error: String },

    /// Remedy: check the path and permissions of the to be generated
    /// configuration file.
    #[error(
        "Unable write a configuration file with default values:\n\
        ---\n\
        {error}"
    )]
    ConfigFileWrite { error: String },

    /// Remedy: restart, or check file permission of the configuration file.
    #[error(
        "Unable to load or parse the (merged)\n\
        configuration file(s):\n\
        ---\n\
        {error}\n\n\
        Note: this error may occur after upgrading\n\
        Tp-Note due to some incompatible configuration\n\
        file changes.\n\
        \n\
        Tp-Note renames and thus disables the last sourced\n\
        configuration file."
    )]
    ConfigFileLoadParse { error: String },

    /// Remedy: restart.
    #[error(
        "Configuration file version mismatch:\n---\n\
        Configuration file version: \'{config_file_version}\'\n\
        Minimum required version: \'{min_version}\'\n\
        \n\
        Tp-Note renames and thus disables the last sourced\n\
        configuration file."
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

    /// Should not happen. Please report this bug.
    #[error("No path to configuration file found.")]
    PathToConfigFileNotFound,

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
