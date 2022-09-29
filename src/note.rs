//! Creates a memory representations of the note by inserting _Tp-Note_'s
//! environment data in some templates. If the note exists on disk already,
//! the memory representation is established be reading the note file with
//! its front matter.

use crate::config::CFG;
use crate::content::Content;
use crate::context::Context;
use crate::error::NoteError;
use crate::error::FRONT_MATTER_ERROR_MAX_LINES;
use crate::filename;
use crate::filename::MarkupLanguage;
use crate::filter::TERA;
use crate::note_error_tera_template;
use parse_hyperlinks::renderer::text_links2html;
#[cfg(feature = "viewer")]
use parse_hyperlinks::renderer::text_rawlinks2html;
#[cfg(feature = "renderer")]
use pulldown_cmark::{html, Options, Parser};
#[cfg(feature = "renderer")]
use rst_parser::parse;
#[cfg(feature = "renderer")]
use rst_renderer::render_html;
use std::default::Default;
use std::fs::File;
use std::fs::OpenOptions;
use std::io;
use std::io::prelude::*;
use std::io::Write;
use std::matches;
use std::path::{Path, PathBuf};
use std::str;
use std::time::SystemTime;
use tera::Tera;

/// The template variable contains the fully qualified path of the `<path>`
/// command line argument. If `<path>` points to a file, the variable contains the
/// file path. If it points to a directory, it contains the directory path, or -
/// if no `path` is given - the current working directory.
pub const TMPL_VAR_PATH: &str = "path";

/// Contains the fully qualified directory path of the `<path>` command line
/// argument.
/// If `<path>` points to a file, the last component (the file name) is omitted.
/// If it points to a directory, the content of this variable is identical to
/// `TMPL_VAR_PATH`,
pub const TMPL_VAR_DIR_PATH: &str = "dir_path";

/// Contains the YAML header (if any) of the clipboard content.
/// Otherwise the empty string.
pub const TMPL_VAR_CLIPBOARD_HEADER: &str = "clipboard_header";

/// If there is a YAML header in the clipboard content, this contains
/// the body only. Otherwise, it contains the whole clipboard content.
pub const TMPL_VAR_CLIPBOARD: &str = "clipboard";

/// Contains the YAML header (if any) of the `stdin` input stream.
/// Otherwise the empty string.
pub const TMPL_VAR_STDIN_HEADER: &str = "stdin_header";

/// If there is a YAML header in the `stdin` input stream, this contains the
/// body only. Otherwise, it contains the whole input stream.
pub const TMPL_VAR_STDIN: &str = "stdin";

/// Contains the default file extension for new note files as defined in the
/// configuration file.
pub const TMPL_VAR_EXTENSION_DEFAULT: &str = "extension_default";

/// Contains the content of the first non empty environment variable
/// `LOGNAME`, `USERNAME` or `USER`.
pub const TMPL_VAR_USERNAME: &str = "username";

/// Contains the user's language tag as defined in
/// [RFC 5646](http://www.rfc-editor.org/rfc/rfc5646.txt).
/// Not to be confused with the UNIX `LANG` environment variable from which
/// this value is derived under Linux/MacOS.
/// Under Windows, the user's language tag is queried through the WinAPI.
/// If defined, the environment variable `TPNOTELANG` overwrites this value
/// (all operating systems).
pub const TMPL_VAR_LANG: &str = "lang";

///  Contains the body of the file the command line option `<path>`
///  points to. Only available in the `TMPL_FROM_TEXT_FILE_CONTENT` template.
///  Only available in the `TMPL_FROM_TEXT_FILE_CONTENT` template.
pub const TMPL_VAR_PATH_FILE_TEXT: &str = "path_file_text";

///  Contains the date of the file the command line option `<path>` points to.
///  The date is represented as an integer the way `std::time::SystemTime`
///  resolves to on the platform. Only available in the
///  `TMPL_FROM_TEXT_FILE_CONTENT` template.
pub const TMPL_VAR_PATH_FILE_DATE: &str = "path_file_date";

/// Prefix prepended to front matter field names when a template variable
/// is generated with the same name.
pub const TMPL_VAR_FM_: &str = "fm_";

/// Contains a Hash Map with all front matter fields. Lists are flattened
/// into a strings.
pub const TMPL_VAR_FM_ALL: &str = "fm_all";

/// All the front matter fields serialized as text, exactly as they appear in
/// the front matter.
pub const TMPL_VAR_FM_ALL_YAML: &str = "fm_all_yaml";

/// By default, the template `TMPL_SYNC_FILENAME` defines the function of
/// of this variable as follows:
/// Contains the value of the front matter field `file_ext` and determines the
/// markup language used to render the document. When the field is missing the
/// markup language is derived from the note's filename extension.
///
/// This is a dynamically generated variable originating from the front matter
/// of the current note. As all front matter variables, it's value is copied as
/// it is without modification.  Here, the only special treatment is, when
/// analyzing the front matter, it is verified, that the value of this variable
/// is registered in one of the `[filename] extensions_*` variables.
pub const TMPL_VAR_FM_FILE_EXT: &str = "fm_file_ext";

/// By default, the template `TMPL_SYNC_FILENAME` defines the function of
/// of this variable as follows:
/// If this variable is defined, the _sort tag_ of the filename is replaced with
/// the value of this variable next time the filename is synchronized.  If not
/// defined, the sort tag of the filename is never changed.
///
/// This is a dynamically generated variable originating from the front matter
/// of the current note. As all front matter variables, it's value is copied as
/// it is without modification.  Here, the only special treatment is, when
/// analyzing the front matter, it is verified, that all the characters of the
/// value of this variable are listed in `[filename] sort_tag_chars`.
pub const TMPL_VAR_FM_SORT_TAG: &str = "fm_sort_tag";

/// Contains the value of the front matter field `no_filename_sync`.  When set
/// to `no_filename_sync:` or `no_filename_sync: true`, the filename
/// synchronisation mechanism is disabled for this note file.  Depreciated
/// in favour of `TMPL_VAR_FM_FILENAME_SYNC`.
pub const TMPL_VAR_FM_NO_FILENAME_SYNC: &str = "fm_no_filename_sync";

/// Contains the value of the front matter field `filename_sync`.  When set to
/// `filename_sync: false`, the filename synchronisation mechanism is
/// disabled for this note file. Default value is `true`.
pub const TMPL_VAR_FM_FILENAME_SYNC: &str = "fm_filename_sync";
/// Default content template used when the command line argument <sanit> is a directory. Can be

/// changed through editing the configuration file.
/// The following variables are  defined:
/// `{{ sanit | stem }}`, `{{ path | stem }}`, `{{ path | ext }}`, `{{ extension_default }}` `{{
/// file | tag }}`, `{{ username }}`, `{{ date }}`, `{{ lang }}`, `{{ dir_path }}`.
/// In addition all environment variables can be used, e.g.  `{{ get_env(name=\"LOGNAME\") }}`
/// When placed in YAML front matter, the filter `| json_encode` must be appended to each variable.
pub const TMPL_NEW_CONTENT: &str = "\
---
title:      {{ dir_path | trim_tag | cut | json_encode }}
subtitle:   {{ 'Note' | json_encode }}
author:     {{ username | json_encode }}
date:       {{ now() | date(format='%Y-%m-%d') | json_encode }}
lang:       {{ lang | json_encode }}
---


";

/// Default filename template for a new note file on disk. It implements the sync criteria for
/// note metadata in front matter and filename.
/// Useful variables in this context are:
/// `{{ title| sanit }}`, `{{ subtitle| sanit }}`, `{{ extension_default }}`,
/// All variables also exist in a `{{ <var>| sanit(alpha) }}` variant: in case its value starts
/// with a number, the string is prepended with `'`.  The first non-numerical variable must be some
/// `{{ <var>| sanit(alpha) }}` variant.
/// Note, as this is filename template, all variables (except `now` and `extension_default` must be
/// filtered by a `sanit` or `sanit(force_alpha=true)` filter.
pub const TMPL_NEW_FILENAME: &str = "\
{{ now() | date(format='%Y%m%d-') }}\
{{ fm_title | sanit(force_alpha=true) }}{% if fm_subtitle | default(value='') | sanit != '' %}--{% endif %}\
{{ fm_subtitle | default(value='') | sanit  }}{{ extension_default | prepend_dot }}\
";

/// Default template used, when the clipboard or the input stream `stdin` contains a string and one
/// the of these strings contains a valid YAML front matter section.
/// The clipboards body is in `{{ clipboard }}`, the header is in `{{ clipboard_header }}`.  The
/// stdin's body is in `{{ stdin }}`, the header is in `{{ stdin_header }}`.
/// First all variables defined in the clipboard's front matter are registered, the ones
/// defined in the input stream `stdin`. The latter can overwrite the former.  One of the front
/// matters must define the `title` variable, which is then available in this template as `{{
/// fm_title }}`.
/// When placed in YAML front matter, the filter `| json_encode` must be
/// appended to each variable.
pub const TMPL_FROM_CLIPBOARD_YAML_CONTENT: &str = "\
---
title:      {{ fm_title | default(value = path|trim_tag) | cut | json_encode }}
subtitle:   {{ fm_subtitle | default(value = 'Note') | cut | json_encode }}
author:     {{ fm_author | default(value=username) | json_encode }}
date:       {{ fm_date | default(value = now()|date(format='%Y-%m-%d')) | json_encode }}
lang:       {{ fm_lang | default(value = lang) | json_encode }}
{% for k, v in fm_all\
 | remove(var='fm_title')\
 | remove(var='fm_subtitle')\
 | remove(var='fm_author')\
 | remove(var='fm_date')\
 | remove(var='fm_lang') %}\
{{ k }}:\t\t{{ v | json_encode }}
{% endfor %}\
---

{{ stdin ~ clipboard }}

";

/// Default filename template used when the stdin or the clipboard contains a string and one of
/// them has a valid YAML header.
pub const TMPL_FROM_CLIPBOARD_YAML_FILENAME: &str = "\
{{ fm_sort_tag | default(value = now() | date(format='%Y%m%d-')) }}\
{{ fm_title | sanit(force_alpha=true) }}\
{% if fm_subtitle | default(value='') | sanit != '' %}--{% endif %}\
{{ fm_subtitle | default(value='') | sanit  }}\
{{ fm_file_ext | default(value = extension_default ) | prepend_dot }}\
";

/// Default template used, when the clipboard or the input stream `stdin` contains a string and
/// this string has no valid YAML front matter section.  The clipboards content is in `{{ clipboard
/// }}`, its truncated version in `{{ clipboard | heading }}` When the clipboard contains a
/// hyperlink in Markdown or reStruncturedText format. See crate `parse-hyperlinks` for details.
/// For example: `[<link-name>](<link-url> "link-title")`, can be accessed with the variables:
/// `{{ clipboard | linkname }}`, `{{ clipboard | linktarget }}` and `{{ clipboard | linkttitle }}`.
pub const TMPL_FROM_CLIPBOARD_CONTENT: &str = "\
{%- set lname = stdin ~ clipboard | linkname -%}
{%- set ok_linkname = lname !=''\
    and not lname is starting_with(\"http\")\
    and not lname is starting_with(\"HTTP\") -%}
---
{% if ok_linkname %}\
title:      {{ stdin ~ clipboard | linkname | cut | json_encode }}
{% else %}\
title:      {{ stdin ~ clipboard | heading | cut | json_encode }}
{% endif %}\
{% if stdin ~ clipboard | linkname !='' and stdin ~ clipboard | cut | linebreaksbr == stdin ~ clipboard | cut %}\
subtitle:   {{ 'URL' | json_encode }}
{% else %}\
subtitle:   {{ 'Note' | json_encode }}
{% endif %}\
author:     {{ username | json_encode }}
date:       {{ now() | date(format='%Y-%m-%d') | json_encode }}
lang:       {{ lang | json_encode }}
---

{{ stdin ~ clipboard }}

";

/// Default filename template used when the stdin ~ clipboard contains a string.
pub const TMPL_FROM_CLIPBOARD_FILENAME: &str = "\
{{ now() | date(format='%Y%m%d-') }}\
{{ fm_title | sanit(force_alpha=true) }}\
{% if fm_subtitle | default(value='') | sanit != '' %}--{% endif %}\
{{ fm_subtitle | default(value='') | sanit  }}{{ extension_default | prepend_dot }}\
";

/// Default template used, when the opened text file (with a known file
/// extension) is missing a YAML front matter section. This template prepends
/// such a section. The template inserts information extracted from the input
/// filename and its creation date.
pub const TMPL_FROM_TEXT_FILE_CONTENT: &str = "\
---
title:      {{ path | stem | split(pat='--') | first | cut | json_encode }}
subtitle:   {{ path | stem | split(pat='--') | nth(n=1) | cut | json_encode }}
author:     {{ username | json_encode }}
date:       {{ path_file_date | date(format='%Y-%m-%d') | json_encode }}
orig_name:  {{ path | filename | json_encode }}
lang:       {{ lang | json_encode }}
---

{{ path_file_text }}
";

/// Default filename template used when the input file (with a known
/// file extension) is missing a YAML front matter section.
/// The text file's sort-tag and file extension are preserved.
pub const TMPL_FROM_TEXT_FILE_FILENAME: &str = "\
{% if path | tag == '' %}{{ path_file_date | date(format='%Y%m%d-') }}\
{% else %}{{ path | tag }}{% endif %}\
{{ fm_title | sanit(force_alpha=true) }}\
{% if fm_subtitle | default(value='') | sanit != '' %}--{% endif %}\
{{ fm_subtitle | default(value='') | sanit  }}\
{{ path | ext | prepend_dot }}\
";

/// Default template used when the command line <path> parameter points to an existing
/// non-`.md`-file. Can be modified through editing the configuration file.
pub const TMPL_ANNOTATE_FILE_CONTENT: &str = "\
---
title:      {{ path | trim_tag | json_encode }}
{% if stdin ~ clipboard | linkname !='' and stdin ~ clipboard | heading == stdin ~ clipboard %}\
subtitle:   {{ 'URL' | json_encode }}
{% else %}\
subtitle:   {{ 'Note' | json_encode }}
{% endif %}\
author:     {{ username | json_encode }}
date:       {{ now() | date(format='%Y-%m-%d') | json_encode }}
lang:       {{ lang | json_encode }}
---

[{{ path | filename }}](<{{ path | filename }}>)
{% if stdin ~ clipboard != '' %}{% if stdin ~ clipboard != stdin ~ clipboard | heading %}
---
{% endif %}
{{ stdin ~ clipboard }}
{% endif %}
";

/// Filename of a new note, that annotates an existing file on disk given in
/// <path>.
pub const TMPL_ANNOTATE_FILE_FILENAME: &str = "\
{{ path | tag }}{{ fm_title | sanit(force_alpha=true) }}\
{% if fm_subtitle | default(value='') | sanit != '' %}--{% endif %}\
{{ fm_subtitle | default(value='') | sanit }}{{ extension_default | prepend_dot }}\
";

/// Default filename template to test, if the filename of an existing note file on disk,
/// corresponds to the note's meta data stored in its front matter. If it is not the case, the
/// note's filename will be renamed.  Can be modified through editing the configuration file.
pub const TMPL_SYNC_FILENAME: &str = "\
{{ fm_sort_tag | default(value = path | tag) }}\
{{ fm_title | default(value='No title') | sanit(force_alpha=true) }}\
{% if fm_subtitle | default(value='') | sanit != '' %}--{% endif %}\
{{ fm_subtitle | default(value='') | sanit  }}\
{{ fm_file_ext | default(value = path | ext) | prepend_dot }}\
";

/// HTML template variable containing the note's body.
/// We could set  
/// `#[cfg(feature = "viewer")]`,
/// but we prefer the same config file structure independent
/// of the enabled features.
pub const TMPL_VAR_NOTE_BODY: &str = "note_body";

/// HTML template variable containing the automatically generated JavaScript
/// code to be included in the HTML rendition.
/// We could set  
/// `#[cfg(feature = "viewer")]`,
/// but we prefer the same config file structure independent
/// of the enabled features.
pub const TMPL_VAR_NOTE_JS: &str = "note_js";

/// HTML template variable used in the error page containing the error message
/// explaining why this page could not be rendered.
/// We could set  
/// `#[cfg(feature = "viewer")]`,
/// but we prefer the same config file structure independent
/// of the enabled features.
#[allow(dead_code)]
pub const TMPL_VAR_NOTE_ERROR: &str = "note_error";

/// HTML template variable used in the error page containing a verbatim
/// HTML rendition with hyperlinks of the erroneous note file.
/// We could set  
/// `#[cfg(feature = "viewer")]`,
/// but we prefer the same config file structure independent
/// of the enabled features.
#[allow(dead_code)]
pub const TMPL_VAR_NOTE_ERRONEOUS_CONTENT: &str = "note_erroneous_content";

/// HTML template to render regular viewer pages.
/// We could set  
/// `#[cfg(feature = "viewer")]`,
/// but we prefer the same config file structure independent
/// of the enabled features.
pub const VIEWER_RENDITION_TMPL: &str = r#"<!DOCTYPE html>
<html lang="{{ fm_lang | default(value='en') }}">
<head>
<meta charset="UTF-8">
<title>{{ fm_title }}</title>
<style>
table, th, td { font-weight: normal; }
table.center {
  margin-left: auto;
  margin-right: auto;
  background-color: #f3f2e4;
  border:1px solid grey;
}
th, td {
  padding: 3px;
  padding-left:15px;
  padding-right:15px;
}
th.key{ color:#444444; text-align:right; }
th.val{
  color:#316128;
  text-align:left;
  font-family:sans-serif;
}
th.keygrey{ color:grey; text-align:right; }
th.valgrey{ color:grey; text-align:left; }
pre { white-space: pre-wrap; }
em { color: #523626; }
a { color: #316128; }
h1 { font-size: 150% }
h2 { font-size: 132% }
h3 { font-size: 115% }
h4, h5, h6 { font-size: 100% }
h1, h2, h3, h4, h5, h6 { color: #263292; font-family:sans-serif; }

</style>
  </head>
  <body>
  <table class="center">
    <tr>
    <th class="key">title:</th>
    <th class="val"><b>{{ fm_title }}</b></th>
  </tr>
    <tr>
    <th class="key">subtitle:</th>
    <th class="val">{{ fm_subtitle | default(value='') }}</th>
  </tr>
    <tr>
    <th class="keygrey">date:</th>
    <th class="valgrey">{{ fm_date | default(value='') }}</th>
  </tr>
  {% for k, v in fm_all| remove(var='fm_title')| remove(var='fm_subtitle')| remove(var='fm_date') %}
    <tr>
    <th class="keygrey">{{ k }}:</th>
    <th class="valgrey">{{ v }}</th>
  </tr>
  {% endfor %}
  </table>
  <div class="note-body">{{ note_body }}</div>
  <script>{{ note_js }}</script>
</body>
</html>
"#;

/// HTML template to render the viewer-error page.
/// We could set  
/// `#[cfg(feature = "viewer")]`,
/// but we prefer the same config file structure independent
/// of the enabled features.
pub const VIEWER_ERROR_TMPL: &str = r#"<!DOCTYPE html>
<html lang="en">
<head>
<meta charset="UTF-8">
<title>Syntax error</title>
<style>
.note-error { color: #523626; }
pre { white-space: pre-wrap; }
a { color: #316128; }
h1, h2, h3, h4, h5, h6 { color: #d3af2c; font-family:sans-serif; }
</style>
</head>
<body>
<h3>Syntax error</h3>
<p> in note file: <pre>{{ path }}</pre><p>
<div class="note-error">
<hr>
<pre>{{ note_error }}</pre>
<hr>
</div>
{{ note_erroneous_content }}
<script>{{ note_js }}</script>
</body>
</html>
"#;

/// HTML template used to render a note into html when the
/// rendition is saved to disk. Similar to `VIEWER_RENDITION_TMPL`
/// but does not inject JavaScript code.
pub const EXPORTER_RENDITION_TMPL: &str = r#"<!DOCTYPE html>
<html lang="{{ fm_lang | default(value='en') }}">
<head>
<meta charset="utf-8">
<title>{{ fm_title }}</title>
<style>
table, th, td { font-weight: normal; }
table.center {
  margin-left: auto;
  margin-right: auto;
  background-color: #f3f2e4;
  border:1px solid grey;
}
th, td {
  padding: 3px;
  padding-left:15px;
  padding-right:15px;
}
th.key{ color:#444444; text-align:right; }
th.val{
  color:#316128;
  text-align:left;
  font-family:sans-serif;
}
th.keygrey{ color:grey; text-align:right; }
th.valgrey{ color:grey; text-align:left; }
pre { white-space: pre-wrap; }
em { color: #523626; }
a { color: #316128; }
h1 { font-size: 150% }
h2 { font-size: 132% }
h3 { font-size: 115% }
h4, h5, h6 { font-size: 100% }
h1, h2, h3, h4, h5, h6 { color: #263292; font-family:sans-serif; }

</style>
  </head>
  <body>
  <table class="center">
    <tr>
    <th class="key">title:</th>
    <th class="val"><b>{{ fm_title }}</b></th>
  </tr>
    <tr>
    <th class="key">subtitle:</th>
    <th class="val">{{ fm_subtitle | default(value='') }}</th>
  </tr>
    <tr>
    <th class="keygrey">date:</th>
    <th class="valgrey">{{ fm_date | default(value='') }}</th>
  </tr>
  {% for k, v in fm_all| remove(var='fm_title')| remove(var='fm_subtitle')| remove(var='fm_date') %}
    <tr>
    <th class="keygrey">{{ k }}:</th>
    <th class="valgrey">{{ v }}</th>
  </tr>
  {% endfor %}
  </table>
  <div class="note-body">{{ note_body }}</div>
</body>
</html>
"#;

#[derive(Debug, PartialEq)]
/// Represents a note.
pub struct Note {
    // Reserved for future use:
    //     /// The front matter of the note.
    //     front_matter: FrontMatter,
    /// Captured environment of _Tp-Note_ that
    /// is used to fill in templates.
    pub context: Context,
    /// The full text content of the note, including
    /// its front matter.
    pub content: Content,
}

#[derive(Debug, Eq, PartialEq)]
/// Represents the front matter of the note.
pub struct FrontMatter {
    pub map: tera::Map<String, tera::Value>,
}

impl TryFrom<&Content> for FrontMatter {
    type Error = NoteError;
    /// Helper function deserializing the front-matter of the note file.
    fn try_from(content: &Content) -> Result<FrontMatter, NoteError> {
        let header = content.borrow_dependent().header;
        Self::try_from(header)
    }
}

impl TryFrom<&str> for FrontMatter {
    type Error = NoteError;
    /// Helper function deserializing the front-matter of the note file.
    fn try_from(header: &str) -> Result<FrontMatter, NoteError> {
        //fn deserialize_header(header: &str) -> Result<FrontMatter, NoteError> {
        if header.is_empty() {
            return Err(NoteError::MissingFrontMatter {
                compulsory_field: CFG.tmpl.compulsory_header_field.to_owned(),
            });
        };

        let map: tera::Map<String, tera::Value> =
            serde_yaml::from_str(header).map_err(|e| NoteError::InvalidFrontMatterYaml {
                front_matter: header
                    .lines()
                    .enumerate()
                    .map(|(n, s)| format!("{:03}: {}\n", n + 1, s))
                    .take(FRONT_MATTER_ERROR_MAX_LINES)
                    .collect::<String>(),
                source_error: e,
            })?;
        let fm = FrontMatter { map };

        // `sort_tag` has additional constrains to check.
        if let Some(tera::Value::String(sort_tag)) = &fm
            .map
            .get(TMPL_VAR_FM_SORT_TAG.trim_start_matches(TMPL_VAR_FM_))
        {
            if !sort_tag.is_empty() {
                // Check for forbidden characters.
                if !sort_tag
                    .trim_start_matches(
                        &CFG.filename.sort_tag_chars.chars().collect::<Vec<char>>()[..],
                    )
                    .is_empty()
                {
                    return Err(NoteError::SortTagVarInvalidChar {
                        sort_tag: sort_tag.to_owned(),
                        sort_tag_chars: CFG.filename.sort_tag_chars.escape_default().to_string(),
                    });
                }
            };
        };

        // `extension` has also additional constrains to check.
        // Is `extension` listed in `CFG.filename.extensions_*`?
        if let Some(tera::Value::String(file_ext)) = &fm
            .map
            .get(TMPL_VAR_FM_FILE_EXT.trim_start_matches(TMPL_VAR_FM_))
        {
            let extension_is_unknown =
                matches!(MarkupLanguage::from(&**file_ext), MarkupLanguage::None);
            if extension_is_unknown {
                return Err(NoteError::FileExtNotRegistered {
                    extension: file_ext.to_owned(),
                    md_ext: CFG.filename.extensions_md.to_owned(),
                    rst_ext: CFG.filename.extensions_rst.to_owned(),
                    html_ext: CFG.filename.extensions_html.to_owned(),
                    txt_ext: CFG.filename.extensions_txt.to_owned(),
                    no_viewer_ext: CFG.filename.extensions_no_viewer.to_owned(),
                });
            }
        };

        Ok(fm)
    }
}

use std::fs;
impl Note {
    /// Constructor that creates a memory representation of an existing note on
    /// disk.
    pub fn from_existing_note(mut context: Context) -> Result<Self, NoteError> {
        let content =
            Content::from_input_with_cr(fs::read_to_string(&context.path).map_err(|e| {
                NoteError::Read {
                    path: context.path.to_path_buf(),
                    source: e,
                }
            })?);

        // Deserialize the note read from disk.
        let fm = FrontMatter::try_from(&content)?;

        if !&CFG.tmpl.compulsory_header_field.is_empty() {
            if let Some(tera::Value::String(header_field)) =
                fm.map.get(&CFG.tmpl.compulsory_header_field)
            {
                if header_field.is_empty() {
                    return Err(NoteError::CompulsoryFrontMatterFieldIsEmpty {
                        field_name: CFG.tmpl.compulsory_header_field.to_owned(),
                    });
                };
            } else {
                return Err(NoteError::MissingFrontMatterField {
                    field_name: CFG.tmpl.compulsory_header_field.to_owned(),
                });
            }
        }

        // Register the raw serialized header text.
        (*context).insert(TMPL_VAR_FM_ALL_YAML, &content.borrow_dependent().header);

        context.insert_front_matter(&fm);

        // Return new note.
        Ok(Self {
            // Reserved for future use:
            //     front_matter: fm,
            context,
            content,
        })
    }

    /// Constructor that prepends a YAML header to an existing text file.
    /// Throws an error if the file has a header.
    pub fn from_text_file(mut context: Context, template: &str) -> Result<Self, NoteError> {
        {
            let mut file = File::open(&context.path)?;
            // Get the file's content.
            let mut raw_text = String::new();
            file.read_to_string(&mut raw_text)?;
            //We keep only the body, if ever there is a header.
            let content = Content::from_input_with_cr(raw_text);
            let header = &content.borrow_dependent().header;
            if !header.is_empty() {
                return Err(NoteError::CannotPrependHeader {
                    existing_header: header
                        .lines()
                        .take(5)
                        .map(|s| s.to_string())
                        .collect::<String>(),
                });
            };
            //We keep the body.
            (*context).insert(TMPL_VAR_PATH_FILE_TEXT, &content.borrow_dependent().body);

            // Get the file's creation date.
            let metadata = file.metadata()?;
            if let Ok(time) = metadata.created() {
                (*context).insert(
                    TMPL_VAR_PATH_FILE_DATE,
                    &time
                        .duration_since(SystemTime::UNIX_EPOCH)
                        .unwrap_or_default()
                        .as_secs(),
                );
            }
        }
        Self::from_content_template(context, template)
    }

    /// Constructor that creates a new note by filling in the content template `template`.
    pub fn from_content_template(mut context: Context, template: &str) -> Result<Self, NoteError> {
        log::trace!(
            "Available substitution variables for content template:\n{:#?}",
            *context
        );

        log::trace!("Applying content template:\n{}", template);

        // render template
        let content = Content::from({
            let mut tera = Tera::default();
            tera.extend(&TERA)?;

            tera.render_str(template, &context)
                .map_err(|e| note_error_tera_template!(e))?
        });

        log::debug!(
            "Rendered content template:\n---\n{}\n---\n{}",
            content.borrow_dependent().header,
            content.borrow_dependent().body.trim()
        );

        // deserialize the rendered template
        let fm = FrontMatter::try_from(&content)?;

        context.insert_front_matter(&fm);

        // Return new note.
        Ok(Self {
            // Reserved for future use:
            //     front_matter: fm,
            context,
            content,
        })
    }

    /// Applies a Tera template to the notes context in order to generate a
    /// sanitized filename that is in sync with the note's meta data stored in
    /// its front matter.
    pub fn render_filename(&self, template: &str) -> Result<PathBuf, NoteError> {
        log::trace!(
            "Available substitution variables for the filename template:\n{:#?}",
            *self.context
        );
        log::trace!("Applying the filename template:\n{}", template);

        // render template
        let mut file_path = self.context.dir_path.to_owned();
        let mut tera = Tera::default();
        tera.extend(&TERA)?;

        match tera.render_str(template, &self.context) {
            Ok(filename) => {
                log::debug!("Rendered filename template:\n{:?}", filename.trim());
                file_path.push(filename.trim());
            }
            Err(e) => {
                return Err(note_error_tera_template!(e));
            }
        }

        Ok(filename::shorten_filename(file_path))
    }

    /// Renders `self` into HTML and saves the result in `export_dir`. If
    /// `export_dir` is the empty string, the directory of `note_path` is
    /// used. `-` dumps the rendition to STDOUT.
    pub fn render_and_write_content(
        &mut self,
        note_path: &Path,
        template: &str,
        export_dir: &Path,
    ) -> Result<(), NoteError> {
        // Determine filename of html-file.
        let mut html_path = PathBuf::new();
        if export_dir
            .as_os_str()
            .to_str()
            .unwrap_or_default()
            .is_empty()
        {
            html_path = note_path
                .parent()
                .unwrap_or_else(|| Path::new(""))
                .to_path_buf();
            let mut html_filename = note_path
                .file_name()
                .unwrap_or_default()
                .to_str()
                .unwrap_or_default()
                .to_string();
            html_filename.push_str(".html");
            html_path.push(PathBuf::from(html_filename.as_str()));
        } else if export_dir.as_os_str().to_str().unwrap_or_default() != "-" {
            html_path = export_dir.to_owned();
            let mut html_filename = note_path
                .file_name()
                .unwrap_or_default()
                .to_str()
                .unwrap_or_default()
                .to_string();
            html_filename.push_str(".html");
            html_path.push(PathBuf::from(html_filename.as_str()));
        } else {
            // `export_dir` points to `-` and `html_path` is empty.
        }

        if html_path
            .as_os_str()
            .to_str()
            .unwrap_or_default()
            .is_empty()
        {
            log::info!("Rendering HTML to STDOUT (`{:?}`)", export_dir);
        } else {
            log::info!("Rendering HTML into: {:?}", html_path);
        };

        // The file extension identifies the markup language.
        let note_path_ext = note_path
            .extension()
            .unwrap_or_default()
            .to_str()
            .unwrap_or_default();

        // Check where to dump output.
        if html_path
            .as_os_str()
            .to_str()
            .unwrap_or_default()
            .is_empty()
        {
            let stdout = io::stdout();
            let mut handle = stdout.lock();

            // Write HTML rendition.
            handle.write_all(self.render_content(note_path_ext, template, "")?.as_bytes())?;
        } else {
            let mut handle = OpenOptions::new()
                .write(true)
                .create(true)
                .open(&html_path)?;
            // Write HTML rendition.
            handle.write_all(self.render_content(note_path_ext, template, "")?.as_bytes())?;
        };
        Ok(())
    }

    #[inline]
    /// First, determines the markup language from the file extension or
    /// the `fm_file_ext` YAML variable, if present.
    /// Then calls the appropriate markup renderer.
    /// Finally the result is rendered with the `VIEWER_RENDITION_TMPL`
    /// template.
    pub fn render_content(
        &mut self,
        // We need the file extension to determine the
        // markup language.
        file_ext: &str,
        // HTML template for this rendition.
        tmpl: &str,
        // If not empty, Javascript code to inject in output.
        java_script_insert: &str,
    ) -> Result<String, NoteError> {
        // Deserialize.

        // Render Body.
        let input = self.content.borrow_dependent().body;

        // If this variable is set, overwrite `file_ext`
        let fm_file_ext = match self.context.get(TMPL_VAR_FM_FILE_EXT) {
            Some(tera::Value::String(fm_file_ext)) => fm_file_ext.as_str(),
            _ => "",
        };

        // Render the markup language.
        let html_output = match MarkupLanguage::from(fm_file_ext).or(MarkupLanguage::from(file_ext))
        {
            #[cfg(feature = "renderer")]
            MarkupLanguage::Markdown => Self::render_md_content(input),
            #[cfg(feature = "renderer")]
            MarkupLanguage::RestructuredText => Self::render_rst_content(input)?,
            MarkupLanguage::Html => input.to_string(),
            _ => Self::render_txt_content(input),
        };

        // Register rendered body.
        self.context.insert(TMPL_VAR_NOTE_BODY, &html_output);

        // Java Script
        self.context.insert(TMPL_VAR_NOTE_JS, java_script_insert);

        let mut tera = Tera::default();
        tera.extend(&TERA)?;
        let html = tera
            .render_str(tmpl, &self.context)
            .map_err(|e| note_error_tera_template!(e))?;
        Ok(html)
    }

    #[inline]
    #[cfg(feature = "renderer")]
    /// Markdown renderer.
    fn render_md_content(markdown_input: &str) -> String {
        // Set up options and parser. Besides the CommonMark standard
        // we enable some useful extras.
        let options = Options::all();
        let parser = Parser::new_ext(markdown_input, options);

        // Write to String buffer.
        let mut html_output: String = String::with_capacity(markdown_input.len() * 3 / 2);
        html::push_html(&mut html_output, parser);
        html_output
    }

    #[inline]
    #[cfg(feature = "renderer")]
    /// RestructuredText renderer.
    fn render_rst_content(rest_input: &str) -> Result<String, NoteError> {
        // Note, that the current rst renderer requires files to end with a new line.
        // <https://github.com/flying-sheep/rust-rst/issues/30>
        let mut rest_input = rest_input.trim_start();
        // The rst parser accepts only exactly one newline at the end.
        while rest_input.ends_with("\n\n") {
            rest_input = &rest_input[..rest_input.len() - 1];
        }
        let document = parse(rest_input.trim_start())
            .map_err(|e| NoteError::RstParse { msg: e.to_string() })?;
        // Write to String buffer.
        let mut html_output: Vec<u8> = Vec::with_capacity(rest_input.len() * 3 / 2);
        let _ = render_html(&document, &mut html_output, false);
        Ok(str::from_utf8(&html_output)?.to_string())
    }

    #[inline]
    /// Renderer for markup languages other than the above.
    fn render_txt_content(other_input: &str) -> String {
        text_links2html(other_input)
    }

    /// When the header can not be deserialized, the content is rendered as
    /// "Error HTML page".
    #[inline]
    #[cfg(feature = "viewer")]
    pub fn render_erroneous_content(
        doc_path: &Path,
        template: &str,
        java_script_insert: &str,
        err: NoteError,
    ) -> Result<String, NoteError> {
        // Render error page providing all information we have.

        let mut context = tera::Context::new();
        let err = err.to_string();
        context.insert(TMPL_VAR_NOTE_ERROR, &err);
        context.insert(TMPL_VAR_PATH, &doc_path.to_str().unwrap_or_default());
        // Java Script
        context.insert(TMPL_VAR_NOTE_JS, &java_script_insert);

        // Read from file.
        let note_erroneous_content = fs::read_to_string(&doc_path).unwrap_or_default();
        // Trim BOM.
        let note_erroneous_content = note_erroneous_content.trim_start_matches('\u{feff}');
        // Render to HTML.
        let note_erroneous_content = text_rawlinks2html(note_erroneous_content);
        // Insert.
        context.insert(TMPL_VAR_NOTE_ERRONEOUS_CONTENT, &note_erroneous_content);

        // Apply template.
        let mut tera = Tera::default();
        tera.extend(&TERA)?;
        let html = tera
            .render_str(template, &context)
            .map_err(|e| note_error_tera_template!(e))?;
        Ok(html)
    }
}

#[cfg(test)]
mod tests {
    use super::Context;
    use super::FrontMatter;
    use serde_json::json;
    use std::path::Path;
    use tera::Value;

    #[test]
    fn test_deserialize() {
        let input = "# document start
        title:     The book
        subtitle:  you always wanted
        author:    It's me
        date:      2020-04-21
        lang:      en
        revision:  '1.0'
        sort_tag:  20200420-21_22
        file_ext:  md
        height:    1.23
        count:     2
        neg:       -1
        flag:      true
        numbers:
          - 1
          - 3
          - 5
        ";

        let mut expected = tera::Map::new();
        expected.insert("title".to_string(), Value::String("The book".to_string()));
        expected.insert(
            "subtitle".to_string(),
            Value::String("you always wanted".to_string()),
        );
        expected.insert("author".to_string(), Value::String("It\'s me".to_string()));
        expected.insert("date".to_string(), Value::String("2020-04-21".to_string()));
        expected.insert("lang".to_string(), Value::String("en".to_string()));
        expected.insert("revision".to_string(), Value::String("1.0".to_string()));
        expected.insert(
            "sort_tag".to_string(),
            Value::String("20200420-21_22".to_string()),
        );
        expected.insert("file_ext".to_string(), Value::String("md".to_string()));
        expected.insert("height".to_string(), json!(1.23)); // Number()
        expected.insert("count".to_string(), json!(2)); // Number()
        expected.insert("neg".to_string(), json!(-1)); // Number()
        expected.insert("flag".to_string(), json!(true)); // Bool()
        expected.insert("numbers".to_string(), json!([1, 3, 5])); // Array()

        let expected_front_matter = FrontMatter { map: expected };

        assert_eq!(expected_front_matter, FrontMatter::try_from(input).unwrap());

        //
        // Is empty.
        let input = "";

        assert!(FrontMatter::try_from(input).is_err());

        //
        // forbidden character `x` in `tag`.
        let input = "# document start
        title: The book
        subtitle: you always wanted
        author: It's me
        sort_tag:    123x4";

        assert!(FrontMatter::try_from(input).is_err());

        //
        // Not registered file extension.
        let input = "# document start
        title: The book
        subtitle: you always wanted
        author: It's me
        sort_tag:    123x4
        file_ext:    xyz";

        assert!(FrontMatter::try_from(input).is_err());
    }

    #[test]
    fn test_register_front_matter() {
        let mut tmp = tera::Map::new();
        tmp.insert("file_ext".to_string(), Value::String("md".to_string())); // String
        tmp.insert("height".to_string(), json!(1.23)); // Number()
        tmp.insert("count".to_string(), json!(2)); // Number()
        tmp.insert("neg".to_string(), json!(-1)); // Number()
        tmp.insert("flag".to_string(), json!(true)); // Bool()
        tmp.insert("numbers".to_string(), json!([1, 3, 5])); // Array([Numbers()..])!
        let mut tmp2 = tmp.clone();

        let mut input1 = Context::from(Path::new("a/b/test.md"));
        let input2 = FrontMatter { map: tmp };

        let mut expected = Context::from(Path::new("a/b/test.md"));
        (*expected).insert("fm_file_ext".to_string(), &json!("md")); // String
        (*expected).insert("fm_height".to_string(), &json!(1.23)); // Number()
        (*expected).insert("fm_count".to_string(), &json!(2)); // Number()
        (*expected).insert("fm_neg".to_string(), &json!(-1)); // Number()
        (*expected).insert("fm_flag".to_string(), &json!(true)); // Bool()
        (*expected).insert("fm_numbers".to_string(), &json!("[1,3,5]")); // String()!
        tmp2.remove("numbers");
        tmp2.insert("numbers".to_string(), json!("[1,3,5]")); // String()!
        (*expected).insert("fm_all".to_string(), &tmp2); // Map()

        input1.insert_front_matter(&input2);
        let result = input1;

        assert_eq!(result, expected);
    }
}
