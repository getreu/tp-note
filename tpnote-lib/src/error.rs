//! Custom error types.

use std::io;
use std::path::PathBuf;
use thiserror::Error;

/// The error `InvalidFrontMatterYaml` prints the front matter section of the
/// note file. This constant limits the number of text lines that are printed.
pub const FRONT_MATTER_ERROR_MAX_LINES: usize = 20;

/// Configuration file related filesystem and syntax errors.
#[derive(Debug, Error)]
pub enum FileError {
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

/// Configuration file related semantic errors.
#[derive(Debug, Error, Clone)]
pub enum LibCfgError {
    /// Remedy: Choose another `sort_tag.extra_separator` character.
    #[error(
        "Configuration file error in section `[filename]`:\n\
        \t`sort_tag.extra_separator=\"{extra_separator}\"\n\
        must not be one of `sort_tag_extra_chars=\"{sort_tag_extra_chars}\"`,\n\
        `0..9`, `a..z` or `{dot_file_marker}`."
    )]
    SortTagExtraSeparator {
        dot_file_marker: char,
        sort_tag_extra_chars: String,
        extra_separator: String,
    },

    /// Remedy: Choose another `extension_default` out of
    /// `extensions[..].0`.
    #[error(
        "Configuration file error in section `[filename]`:\n\
        \t`extension_default=\"{extension_default}\"\n\
        must not be one of:`\n\
        \t{extensions}."
    )]
    ExtensionDefault {
        extension_default: String,
        extensions: String,
    },

    /// Remedy: Insert `sort_tag.separator` in `sort_tag.extra_chars`.
    #[error(
        "Configuration file error in section `[filename]`:\n\
        All characters in `sort_tag.separator=\"{separator}\"\n\
        must be in the set `sort_tag.extra_chars=\"{chars}\"`,\n\
        or in `0..9`, `a..z``\n\
        must NOT start with `{dot_file_marker}`."
    )]
    SortTagSeparator {
        dot_file_marker: char,
        chars: String,
        separator: String,
    },

    /// Remedy: Choose a `copy_counter_extra_separator` in the set.
    #[error(
        "Configuration file error in section `[filename]`:\n\
        `copy_counter_extra_separator=\"{extra_separator}\"`\n\
        must be one of: \"{chars}\""
    )]
    CopyCounterExtraSeparator {
        chars: String,
        extra_separator: String,
    },

    /// Remedy: check the configuration file variable `tmpl.filter_assert_preconditions`.
    #[error("choose one of: `IsDefined`, `IsString`, `IsNumber`, `IsStringOrNumber`, `IsBool`, `IsValidSortTag`")]
    ParseAssertPrecondition,

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

    /// Remedy: correct the variable by choosing one the available themes.
    #[error(
        "Configuration file error in section `[tmp_html]` in line:\n\
        \t{var} = \"{value}\"\n\
        The theme must be one of the following set:\n\
        {available}"
    )]
    ThemeName {
        var: String,
        value: String,
        available: String,
    },
}

#[derive(Debug, Error)]
/// Error type returned form methods in or related to the `note` module.
pub enum NoteError {
    /// Remedy: make sure, that a file starting with `path` exists.
    #[error("<NONE FOUND: {path}...>")]
    CanNotExpandShorthandLink { path: String },

    /// Remedy: report this error. It should not happen.
    #[error("Can not prepend header. File has one already: \n{existing_header}")]
    CannotPrependHeader { existing_header: String },

    #[error(transparent)]
    File(#[from] FileError),

    /// Remedy: remove invalid characters.
    #[error(
        "The `sort_tag` header variable contains invalid\n\
         character(s):\n\n\
         \t---\n\
         \tsort_tag: {sort_tag}\n\
         \t---\n\n\
         Only the characters: \"{sort_tag_extra_chars}\", `0..9`\n\
         and `a..z` (maximum {filename_sort_tag_letters_in_succession_max} in \
         succession) are allowed."
    )]
    FrontMatterFieldIsInvalidSortTag {
        sort_tag: String,
        sort_tag_extra_chars: String,
        filename_sort_tag_letters_in_succession_max: u8,
    },

    /// Remedy: index the compound type?
    #[error(
        "The type of the front matter field `{field_name}:`\n\
         must not be a compound type. Use a simple type, \n\
         i.e. `String`, `Number` or `Bool` instead. Example:\n\
         \n\
         \t~~~~~~~~~~~~~~\n\
         \t---\n\
         \t{field_name}: My simple type\n\
         \t---\n\
         \tsome text\n\
         \t~~~~~~~~~~~~~~"
    )]
    FrontMatterFieldIsCompound { field_name: String },

    /// Remedy: try to enclose with quotes.
    #[error(
        "The (sub)type of the front matter field `{field_name}:`\n\
         must be a non empty `String`. Example:\n\
         \n\
         \t~~~~~~~~~~~~~~\n\
         \t---\n\
         \t{field_name}: My string\n\
         \t---\n\
         \tsome text\n\
         \t~~~~~~~~~~~~~~"
    )]
    FrontMatterFieldIsEmptyString { field_name: String },

    /// Remedy: try to remove possible quotes.
    #[error(
        "The (sub)type of the front matter field `{field_name}:`\n\
         must be `Bool`. Example:\n\
         \n\
         \t~~~~~~~~~~~~~~\n\
         \t---\n\
         \t{field_name}: false\n\
         \t---\n\
         \tsome text\n\
         \t~~~~~~~~~~~~~~\n\
         \n\
         Hint: try to remove possible quotes."
    )]
    FrontMatterFieldIsNotBool { field_name: String },

    /// Remedy: try to remove possible quotes.
    #[error(
        "The (sub)type of the front matter field `{field_name}:`\n\
         must be `Number`. Example:\n\
         \n\
         \t~~~~~~~~~~~~~~\n\
         \t---\n\
         \t{field_name}: 142\n\
         \t---\n\
         \tsome text\n\
         \t~~~~~~~~~~~~~~\n\
         \n\
         Hint: try to remove possible quotes."
    )]
    FrontMatterFieldIsNotNumber { field_name: String },

    /// Remedy: try to enclose with quotes.
    #[error(
        "The (sub)type of the front matter field `{field_name}:`\n\
         must be `String`. Example:\n\
         \n\
         \t~~~~~~~~~~~~~~\n\
         \t---\n\
         \t{field_name}: My string\n\
         \t---\n\
         \tsome text\n\
         \t~~~~~~~~~~~~~~\n\
         \n\
         Hint: try to enclose with quotes."
    )]
    FrontMatterFieldIsNotString { field_name: String },

    /// Remedy: correct the front matter variable `file_ext`.
    #[error(
        "The file extension:\n\
        \t---\n\
        \tfile_ext: {extension}\n\
        \t---\n\
        is not registered as Tp-Note file in\n\
        your configuration file:\n\
        \t{extensions}\n\
        \n\
        Choose one of the listed above or add more extensions to the\n\
        `filename.extensions` variable in your configuration file."
    )]
    FrontMatterFieldIsNotTpnoteExtension {
        extension: String,
        extensions: String,
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
    FrontMatterFieldMissing { field_name: String },

    /// Remedy: check front matter delimiters `----`.
    #[error(
        "The document (or template) has no front matter\n\
         section. Is one `---` missing?\n\n\
         \t~~~~~~~~~~~~~~\n\
         \t---\n\
         \t{compulsory_field}: My note\n\
         \t---\n\
         \tsome text\n\
         \t~~~~~~~~~~~~~~\n\
         \n\
         Please correct the front matter if this is\n\
         supposed to be a Tp-Note file. Ignore otherwise."
    )]
    FrontMatterMissing { compulsory_field: String },

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

    /// Remedy: correct link path.
    #[error("<INVALID: {path}>")]
    InvalidLocalPath { path: String },

    #[error(transparent)]
    Io(#[from] std::io::Error),

    #[error(transparent)]
    ParseLanguageCode(#[from] LibCfgError),

    /// Remedy: check the file permission of the note file.
    #[error("Can not read file:\n\t {path:?}\n{source}")]
    Read { path: PathBuf, source: io::Error },

    /// Remedy: check reStructuredText syntax.
    #[error("Can not parse reStructuredText input:\n{msg}")]
    #[cfg(feature = "renderer")]
    RstParse { msg: String },

    /// Remedy: restart with `--debug trace`.
    #[error(
        "Tera error:\n\
         {source}"
    )]
    Tera {
        #[from]
        source: tera::Error,
    },

    /// Remedy: check the syntax of the Tera template in the configuration file.
    #[error(
        "Tera template error in configuration file\n\
        variable \"{template_str}\":\n {source_str}"
    )]
    TeraTemplate {
        source_str: String,
        template_str: String,
    },

    #[error(transparent)]
    Utf8Conversion {
        #[from]
        source: core::str::Utf8Error,
    },
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
