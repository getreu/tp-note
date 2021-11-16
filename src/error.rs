//! Custom error types.

use crate::process_ext::ChildExtError;
use std::io;
use std::path::PathBuf;
use std::process::ExitStatus;
use thiserror::Error;

#[derive(Debug, Error)]
/// Error arising in the `workflow` and `main` module.
pub enum WorkflowError {
    /// Remedy: check <path> to note file.
    #[error("Can not export. No note file found.")]
    ExportsNeedsNoteFile,

    /// Remedy: restart with `--debug trace`.
    #[error("Failed to render template (cf. `{tmpl_name}` in configuration file)!\n{source}")]
    Template {
        tmpl_name: String,
        source: NoteError,
    },

    #[error(transparent)]
    MissingFrontMatter { source: NoteError },

    #[error(transparent)]
    MissingFrontMatterField { source: NoteError },

    #[error(transparent)]
    InvalidFrontMatterYaml { source: NoteError },

    #[error(transparent)]
    InvalidClipboardYaml { source: NoteError },

    #[error(transparent)]
    SortTagVarInvalidChar { source: NoteError },

    #[error(transparent)]
    FileExtNotRegistered { source: NoteError },

    #[error(transparent)]
    Note {
        #[from]
        source: NoteError,
    },

    #[error(transparent)]
    File(#[from] FileError),

    #[error(transparent)]
    Io(#[from] std::io::Error),
}

/// Error related to the filesystem and to invoking external applications.
#[derive(Debug, Error)]
pub enum FileError {
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
        For now, Tp-Note backs up the existing configuration\n\
        file and next time it starts, it will create a new one\n\
        with default values."
    )]
    ConfigFileLoadParseWrite { error: String },

    /// Remedy: restart.
    #[error(
        "Configuration file version mismatch:\n---\n\
        Configuration file version: \'{config_file_version}\'\n\
        Minimum required configuration file version: \'{min_version}\'\n\
        \n\
        For now, Tp-Note backs up the existing configuration\n\
        file and next time it starts, it will create a new one\n\
        with default values."
    )]
    ConfigFileVersionMismatch {
        config_file_version: String,
        min_version: String,
    },

    /// Remedy: restart.
    #[cfg(not(test))]
    #[error(
        "Configuration file error in section `[filename]`:\n\
        `sort_tag_chars=\"{chars}\"` must not contain:\n\
        `sort_tag_extra_separator=\"{extra_separator}\"`"
    )]
    ConfigFileSortTag {
        chars: String,
        extra_separator: String,
    },

    /// Remedy: restart.
    #[cfg(not(test))]
    #[error(
        "Configuration file error in section `[filename]`:\n\
        `copy_counter_extra_separator=\"{extra_separator}\"`\n\
        must be one of: \"{chars}\""
    )]
    ConfigFileCopyCounter {
        chars: String,
        extra_separator: String,
    },

    /// Should not happen. Please report this bug.
    #[error("No path to configuration file found.")]
    PathToConfigFileNotFound,

    /// Should not happen. Please report this bug.
    #[error("Configuration file not found.")]
    ConfigFileNotFound,

    /// Remedy: delete all files in configuration file directory.
    #[error(
        "Can not find unused filename in directory:\n\
        \t{directory:?}\n\
        (only `COPY_COUNTER_MAX` copies are allowed)."
    )]
    NoFreeFileName { directory: PathBuf },

    /// Remedy: check file permission.
    #[error("Can not write file:\n{path:?}\n{source_str}")]
    Write { path: PathBuf, source_str: String },

    /// Should not happen. Please report this bug.
    #[error("Can not convert path to UFT8:\n{path:?}")]
    PathNotUtf8 { path: PathBuf },

    /// Remedy: check the configuration file variables `[app_args] editor`
    /// and `[app_args] browser`.
    #[error("Error executing external application:")]
    ChildExt {
        #[from]
        source: ChildExtError,
    },

    /// Remedy: check the configuration file variable `[app_args] editor`.
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

    /// Remedy: check the configuration file variable `[app_args] editor`.
    #[error(
        "None of the following external applications can be found on your system:\n\
        \t{app_list:?}\n\
        \n\
        Install one of the listed applications on your system -or-\n\
        register some already installed application in the variable \n\
        `{var_name}` in Tp-Note's configuration file."
    )]
    NoApplicationFound {
        app_list: Vec<String>,
        var_name: String,
    },

    #[error(transparent)]
    Io(#[from] std::io::Error),

    #[error(transparent)]
    Serialize(#[from] toml::ser::Error),

    #[error(transparent)]
    Deserialize(#[from] toml::de::Error),
}

/// The error `InvalidFrontMatterYaml` prints the front matter section of the
/// note file. This constant limits the number of text lines that are printed.
pub const FRONT_MATTER_ERROR_MAX_LINES: usize = 20;

/// Macro to construct a `NoteError::TeraTemplate from a `Tera::Error` .
#[macro_export]
macro_rules! note_error_tera_template {
    ($e:ident) => {
        NoteError::TeraTemplate {
            source_str: std::error::Error::source(&$e)
                .unwrap_or(&tera::Error::msg(""))
                .to_string()
                // Remove useless information.
                .trim_end_matches("in context while rendering '__tera_one_off'")
                .to_string(),
        }
    };
}

#[derive(Debug, Error)]
/// Error type returned form methods in or related to the `note` module.
pub enum NoteError {
    /// Remedy: check the file permission of the note file.
    #[error("Can not read file:\n\t {path:?}\n{source}")]
    Read { path: PathBuf, source: io::Error },

    /// Remedy: check the syntax of the Tera template in the configuration file.
    #[error("Tera template error: {source_str}")]
    TeraTemplate { source_str: String },

    /// Remedy: restart with `--debug trace`.
    #[error(
        "Tera error:\n\
         {source}"
    )]
    Tera {
        #[from]
        source: tera::Error,
    },

    /// Remedy: add the missing field in the note's front matter.
    #[error(
        "The document is missing a `{field_name}:` field in its front matter:\n\
         \n\
         \t~~~~~~~~~~~~~~\n\
         \t---\n\
         \t{field_name}: \"My note\"\n\
         \t---\n\
         \tsome text\n\
         \t~~~~~~~~~~~~~~\n\
         \n\
         Please correct the front matter if this is supposed \
         to be a Tp-Note file. Ignore otherwise."
    )]
    MissingFrontMatterField { field_name: String },

    /// Remedy: check YAML syntax in the note's front matter.
    #[error(
        "Can not parse front matter:\n\
         \n\
         {front_matter}\
         \n\
         {source_error}"
    )]
    InvalidFrontMatterYaml {
        front_matter: String,
        source_error: serde_yaml::Error,
    },

    /// Remedy: check YAML syntax in the input stream's front matter.
    #[error(
        "Invalid YAML field(s) in the `stdin` input stream data found:\n\
        {source_str}"
    )]
    InvalidStdinYaml { source_str: String },

    /// Remedy: check YAML syntax in the clipboard's front matter.
    #[error(
        "Invalid YAML field(s) in the clipboard data found:\n\
        {source_str}"
    )]
    InvalidClipboardYaml { source_str: String },

    /// Remedy: check front matter delimiters `----`.
    #[error(
        "The document (or template) has no front matter section.\n\
         Is one `---` missing?\n\n\
         \t~~~~~~~~~~~~~~\n\
         \t---\n\
         \t{compulsory_field}: \"My note\"\n\
         \t---\n\
         \tsome text\n\
         \t~~~~~~~~~~~~~~\n\
         \n\
         Please correct the front matter if this is supposed \
         to be a Tp-Note file. Ignore otherwise."
    )]
    MissingFrontMatter { compulsory_field: String },

    /// Remedy: remove invalid characters.
    #[error(
        "The `sort_tag` header variable contains invalid \
         character(s):\n\n\
         \t---\n\
         \tsort_tag = \"{sort_tag}\"\n\
         \t---\n\n\
         Only the characters: \"{sort_tag_chars}\" are allowed here."
    )]
    SortTagVarInvalidChar {
        sort_tag: String,
        sort_tag_chars: String,
    },

    /// Remedy: correct the front matter variable `file_ext`.
    #[error(
        "The file extension:\n\
        \t---\n\
        \tfile_ext=\"{extension}\"\n\
        \t---\n\
        is not registered as a valid\n\
        Tp-Note-file in the `[filename] extensions_*` variables\n\
        in your configuration file:\n\
        \t{md_ext:?}\n\
        \t{rst_ext:?}\n\
        \t{html_ext:?}\n\
        \t{txt_ext:?}\n\
        \t{no_viewer_ext:?}\n\
        \n\
        Choose one of the above list or add more extensions to the\n\
        `[filename] extensions_*` variables in your configuration file."
    )]
    FileExtNotRegistered {
        extension: String,
        md_ext: Vec<String>,
        rst_ext: Vec<String>,
        html_ext: Vec<String>,
        txt_ext: Vec<String>,
        no_viewer_ext: Vec<String>,
    },

    /// Remedy: check reStructuredText syntax.
    #[error("Can not parse reStructuredText input:\n{msg}")]
    #[cfg(feature = "renderer")]
    RstParse { msg: String },

    #[error(transparent)]
    Utf8Conversion {
        #[from]
        source: core::str::Utf8Error,
    },

    #[error(transparent)]
    Io(#[from] std::io::Error),
}
