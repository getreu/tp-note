//! Custom error types.
use std::path::PathBuf;
use std::process::ExitStatus;
use thiserror::Error;
use tpnote_lib::error::FileError;
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
    File(#[from] FileError),

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
        "Can not backup and delete the erroneous configuration file:\n\
        ---\n\
        {error}\n\n\
        Please do it manually."
    )]
    ConfigFileBackup { error: String },

    /// Remedy: restart, or check file permission of the configuration file.
    #[error(
        "Unable to load, parse or write the configuration file:\n\
        ---\n\
        {error}\n\n\
        Note: this error may occur after upgrading Tp-Note due\n\
        to some incompatible configuration file changes.\n\
        \n\
        Tp-Note backs up the existing configuration\n\
        file and creates a new one with default values."
    )]
    ConfigFileLoadParseWrite { error: String },

    /// Remedy: restart.
    #[error(
        "Configuration file version mismatch:\n---\n\
        Configuration file version: \'{config_file_version}\'\n\
        Minimum required configuration file version: \'{min_version}\'\n\
        \n\
        Tp-Note backs up the existing configuration\n\
        file and creates a new one with default values."
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
        "The external application did not terminate gracefully: {code}\n\
         \n\
         Edit the variable `{var_name}` in Tp-Note's configuration file\n\
         and correct the following:\n\
         \t{args:?}"
    )]
    ApplicationReturn {
        code: ExitStatus,
        var_name: String,
        args: Vec<String>,
    },

    /// Remedy: check the configuration file variable `app_args.editor`
    /// or `app_args.browser` depending on the displayed variable name.
    #[error(
        "Can not find any external application listend in `{var_name}`:\n\
        \t{app_list:?}\n\
        \n\
        Install one of the listed applications on your system -or-\n\
        register some already installed application in Tp-Note's \n\
        configuration file or in the corresponding environment variable."
    )]
    NoApplicationFound {
        app_list: Vec<Vec<String>>,
        var_name: String,
    },

    #[error(transparent)]
    Io(#[from] std::io::Error),

    #[error(transparent)]
    IoRef(#[from] &'static std::io::Error),

    #[error(transparent)]
    Serialize(#[from] toml::ser::Error),

    #[error(transparent)]
    Deserialize(#[from] toml::de::Error),
}
