//! Creates a memory representations of the note by inserting _Tp-Note_'s
//! environment data in some templates. If the note exists on disk already,
//! the memory representation is established be reading the note file with
//! its front matter.

use crate::config2::CFG2;
use crate::config2::TMPL_VAR_FM_;
use crate::config2::TMPL_VAR_FM_ALL_YAML;
use crate::config2::TMPL_VAR_FM_FILE_EXT;
use crate::config2::TMPL_VAR_FM_SORT_TAG;
use crate::config2::TMPL_VAR_NOTE_BODY;
#[cfg(feature = "viewer")]
use crate::config2::TMPL_VAR_NOTE_ERRONEOUS_CONTENT;
#[cfg(feature = "viewer")]
use crate::config2::TMPL_VAR_NOTE_ERROR;
use crate::config2::TMPL_VAR_NOTE_JS;
#[cfg(feature = "viewer")]
use crate::config2::TMPL_VAR_PATH;
use crate::config2::TMPL_VAR_PATH_FILE_DATE;
use crate::config2::TMPL_VAR_PATH_FILE_TEXT;
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
        let cfg2 = CFG2.read().unwrap();
        //fn deserialize_header(header: &str) -> Result<FrontMatter, NoteError> {
        if header.is_empty() {
            return Err(NoteError::MissingFrontMatter {
                compulsory_field: (*cfg2).tmpl.compulsory_header_field.to_owned(),
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
                        &cfg2.filename.sort_tag_chars.chars().collect::<Vec<char>>()[..],
                    )
                    .is_empty()
                {
                    return Err(NoteError::SortTagVarInvalidChar {
                        sort_tag: sort_tag.to_owned(),
                        sort_tag_chars: cfg2.filename.sort_tag_chars.escape_default().to_string(),
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
                    md_ext: cfg2.filename.extensions_md.to_owned(),
                    rst_ext: cfg2.filename.extensions_rst.to_owned(),
                    html_ext: cfg2.filename.extensions_html.to_owned(),
                    txt_ext: cfg2.filename.extensions_txt.to_owned(),
                    no_viewer_ext: cfg2.filename.extensions_no_viewer.to_owned(),
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
        let cfg2 = CFG2.read().unwrap();

        let content =
            Content::from_input_with_cr(fs::read_to_string(&context.path).map_err(|e| {
                NoteError::Read {
                    path: context.path.to_path_buf(),
                    source: e,
                }
            })?);

        // Deserialize the note read from disk.
        let fm = FrontMatter::try_from(&content)?;

        if !(*cfg2).tmpl.compulsory_header_field.is_empty() {
            if let Some(tera::Value::String(header_field)) =
                fm.map.get(&(*cfg2).tmpl.compulsory_header_field)
            {
                if header_field.is_empty() {
                    return Err(NoteError::CompulsoryFrontMatterFieldIsEmpty {
                        field_name: (*cfg2).tmpl.compulsory_header_field.to_owned(),
                    });
                };
            } else {
                return Err(NoteError::MissingFrontMatterField {
                    field_name: (*cfg2).tmpl.compulsory_header_field.to_owned(),
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
    /// Finally the result is rendered with the `HTML_VIEWER_TMPL`
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
