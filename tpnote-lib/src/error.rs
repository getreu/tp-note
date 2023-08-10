//! Custom error types.

use std::io;
use std::path::PathBuf;
use thiserror::Error;

/// The error `InvalidFrontMatterYaml` prints the front matter section of the
/// note file. This constant limits the number of text lines that are printed.
pub const FRONT_MATTER_ERROR_MAX_LINES: usize = 20;

/// Error related to the filesystem and to invoking external applications.
#[derive(Debug, Error)]
pub enum LibCfgFileError {
    /// Remedy: restart.
    #[cfg(not(test))]
    #[error(
        "Configuration file error in section `[filename]`:\n\
        `sort_tag_extra_separator=\"{extra_separator}\"\n\
        must not be one of `sort_tag_chars=\"{chars}\"`\n\
        or `{char}`"
    )]
    ConfigFileSortTag {
        char: char,
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

    #[error(transparent)]
    Io(#[from] std::io::Error),

    #[error(transparent)]
    Serialize(#[from] toml::ser::Error),

    #[error(transparent)]
    Deserialize(#[from] toml::de::Error),
}

/// Error related to configuration deserialization.
#[derive(Debug, Error, Clone)]
pub enum LibCfgError {
    /// Remedy: check the configuration file variable `arg_default.export_link_rewriting`.
    #[error("choose one of: `off`, `short` or `long`")]
    ParseLocalLinkKind,

    /// Remedy: check the ISO 639-1 codes in the configuration variable
    /// `tmpl.filter_get_lang` and make sure that they are supported, by
    /// checking `tpnote -V`.
    #[error(
        "The ISO 639-1 language subtag `{language_code}`\n\
         in the configuration file variable\n\
         `tmpl.filter_get_lang` or in the environment\n\
         variable `TPNOTE_LANG_DETECTION` is not supported.\n\
         All listed codes must be part of the set:\n\
         {all_langs}."
    )]
    ParseLanguageCode {
        language_code: String,
        all_langs: String,
    },
    /// Remedy: add one more ISO 639-1 code in the configuration variable
    /// `tmpl.filter_get_lang` (or in `TPNOTE_LANG_DETECTION`) and make
    /// sure that the code is supported, by checking `tpnote -V`.
    #[error(
        "Not enough languages to choose from.\n\
         The list of ISO 639-1 language subtags\n\
         currently contains only one item: `{language_code}`.\n\
         Add one more language to the configuration \n\
         file variable `tmpl.filter_get_lang` or to the\n\
         environment variable `TPNOTE_LANG_DETECTION`\n\
         to prevent this error from occurring."
    )]
    NotEnoughLanguageCodes { language_code: String },
}

#[derive(Debug, Error)]
/// Error type returned form methods in or related to the `note` module.
pub enum NoteError {
    /// Remedy: check the file permission of the note file.
    #[error("Can not read file:\n\t {path:?}\n{source}")]
    Read { path: PathBuf, source: io::Error },

    /// Remedy: report this error. It should not happen.
    #[error("Can not prepend header. File has one already: \n{existing_header}")]
    CannotPrependHeader { existing_header: String },

    /// Remedy: check the syntax of the Tera template in the configuration file.
    #[error(
        "Tera template error in configuration file\n\
        variable \"{template_str}\":\n {source_str}"
    )]
    TeraTemplate {
        source_str: String,
        template_str: String,
    },

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
        "The document is missing a `{field_name}:`\n\
         field in its front matter:\n\
         \n\
         \t~~~~~~~~~~~~~~\n\
         \t---\n\
         \t{field_name}: \"My note\"\n\
         \t---\n\
         \tsome text\n\
         \t~~~~~~~~~~~~~~\n\
         \n\
         Please correct the front matter if this is\n\
         supposed to be a Tp-Note file. Ignore otherwise."
    )]
    MissingFrontMatterField { field_name: String },

    /// Remedy: enter a string.
    #[error(
        "The value of the front matter field `{field_name}:`\n\
         must be a non empty string."
    )]
    CompulsoryFrontMatterFieldIsEmpty { field_name: String },

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
        "Invalid YAML field(s) in the {tmpl_var} input\n\
        stream data found:\n\
        {source_str}"
    )]
    InvalidInputYaml {
        tmpl_var: String,
        source_str: String,
    },

    /// Remedy: check front matter delimiters `----`.
    #[error(
        "The document (or template) has no front matter\n\
         section. Is one `---` missing?\n\n\
         \t~~~~~~~~~~~~~~\n\
         \t---\n\
         \t{compulsory_field}: \"My note\"\n\
         \t---\n\
         \tsome text\n\
         \t~~~~~~~~~~~~~~\n\
         \n\
         Please correct the front matter if this is\n\
         supposed to be a Tp-Note file. Ignore otherwise."
    )]
    MissingFrontMatter { compulsory_field: String },

    /// Remedy: remove invalid characters.
    #[error(
        "The `sort_tag` header variable contains invalid\n\
         character(s):\n\n\
         \t---\n\
         \tsort_tag = \"{sort_tag}\"\n\
         \t---\n\n\
         Only the characters: \"{sort_tag_chars}\"\n\
         are allowed here."
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
        is not registered as a valid Tp-Note-file in\n\
        the `filename.extensions_*` variables in your\n\
        configuration file:\n\
        \t{md_ext:?}\n\
        \t{rst_ext:?}\n\
        \t{html_ext:?}\n\
        \t{txt_ext:?}\n\
        \t{no_viewer_ext:?}\n\
        \n\
        Choose one of the listed above or add more\n\
        extensions to the `filename.extensions_*`\n\
        variables in your configuration file."
    )]
    FileExtNotRegistered {
        extension: Box<String>,
        md_ext: Box<Vec<String>>,
        rst_ext: Box<Vec<String>>,
        html_ext: Box<Vec<String>>,
        txt_ext: Box<Vec<String>>,
        no_viewer_ext: Box<Vec<String>>,
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
    File(#[from] LibCfgFileError),

    #[error(transparent)]
    Io(#[from] std::io::Error),

    #[error(transparent)]
    ParseLanguageCode(#[from] LibCfgError),
}

/// Macro to construct a `NoteError::TeraTemplate from a `Tera::Error` .
#[macro_export]
macro_rules! note_error_tera_template {
    ($e:ident, $t:expr) => {
        NoteError::TeraTemplate {
            source_str: std::error::Error::source(&$e)
                .unwrap_or(&tera::Error::msg(""))
                .to_string()
                // Remove useless information.
                .trim_end_matches("in context while rendering '__tera_one_off'")
                .to_string(),
            template_str: $t,
        }
    };
}
