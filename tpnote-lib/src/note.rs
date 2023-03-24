//! Tp-Note's low level API, creating a memory representation of a  
//! note file by inserting _Tp-Note_'s
//! environment data in some templates. If the note exists on disk already,
//! the memory representation is established be reading the note file and
//! parsing its front matter.
//! NB: The high level API is in the module `tpnote_lib::workflow`.

use crate::config::LocalLinkKind;
use crate::config::LIB_CFG;
use crate::config::TMPL_HTML_VAR_NOTE_BODY_HTML;
use crate::config::TMPL_HTML_VAR_NOTE_CSS;
use crate::config::TMPL_HTML_VAR_NOTE_CSS_PATH;
use crate::config::TMPL_HTML_VAR_NOTE_CSS_PATH_VALUE;
use crate::config::TMPL_VAR_FM_FILE_EXT;
use crate::config::TMPL_VAR_NOTE_BODY_TEXT;
use crate::config::TMPL_VAR_NOTE_FILE_DATE;
use crate::config::TMPL_VAR_NOTE_FM_TEXT;
use crate::content::Content;
use crate::context::Context;
use crate::error::NoteError;
use crate::filename::NotePath;
use crate::filename::NotePathBuf;
use crate::filter::TERA;
use crate::front_matter::FrontMatter;
#[cfg(feature = "renderer")]
use crate::highlight::SyntaxPreprocessor;
use crate::html::rewrite_links;
use crate::html::HTML_EXT;
use crate::markup_language::MarkupLanguage;
use crate::note_error_tera_template;
use crate::template::TemplateKind;
use parse_hyperlinks::renderer::text_links2html;
#[cfg(feature = "renderer")]
use pulldown_cmark::{html, Options, Parser};
#[cfg(feature = "renderer")]
use rst_parser::parse;
#[cfg(feature = "renderer")]
use rst_renderer::render_html;
use std::collections::HashSet;
use std::default::Default;
use std::fs::File;
use std::fs::OpenOptions;
use std::io;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::str;
use std::sync::{Arc, RwLock};
use std::time::SystemTime;
use tera::Tera;

#[derive(Debug, PartialEq)]
/// Represents a note.
pub struct Note<T> {
    /// Captured environment of _Tp-Note_ that
    /// is used to fill in templates.
    pub context: Context,
    /// The full text content of the note, including
    /// its front matter.
    pub content: T,
    /// 1. The `ContentString`'s header is deserialized into `FrontMatter`.
    /// 2. `FrontMatter` is stored in `Context` with some environment data.
    /// 3. `Context` data is filled in some filename template.
    /// 4. The result is stored in `rendered_filename`. This field equals to
    ///    `PathBuf::new()` until `self.render_filename` is called.
    pub rendered_filename: PathBuf,
}

use std::fs;
impl<T: Content> Note<T> {
    /// Constructor creating a memory representation of the existing note
    /// on disk.
    ///
    /// Contract: `template_kind` should be one of:
    /// * `TemplateKind::SyncFilename`,
    /// * `TemplateKind::None` or
    /// * `TemplateKind::FromTextFile`.
    ///
    /// Panics otherwise. Use `Note::from_content_template()` in those cases.
    ///
    /// This adds the following variables to the context:
    /// * `TMPL_VAR_NOTE_FM_TEXT`,
    /// * `TMPL_VAR_NOTE_BODY_TEXT`,    
    /// * `TMPL_VAR_NOTE_FILE_DATE` (only if file can be opened),
    /// * all front matter variables (see `FrontMatter::try_from_content()`)
    ///
    pub fn from_text_file(
        mut context: Context,
        content: T,
        template_kind: TemplateKind,
    ) -> Result<Self, NoteError> {
        // Register context variables:
        // Register the raw serialized header text.
        let header = &content.header();
        (*context).insert(TMPL_VAR_NOTE_FM_TEXT, &header);
        //We also keep the body.
        let body = content.body();
        (*context).insert(TMPL_VAR_NOTE_BODY_TEXT, &body);

        // Get the file's creation date. Fail silently.
        if let Ok(file) = File::open(&context.path) {
            if let Ok(metadata) = file.metadata() {
                if let Ok(time) = metadata.created() {
                    (*context).insert(
                        TMPL_VAR_NOTE_FILE_DATE,
                        &time
                            .duration_since(SystemTime::UNIX_EPOCH)
                            .unwrap_or_default()
                            .as_secs(),
                    );
                }
            }
        }

        if matches!(template_kind, TemplateKind::FromTextFile) && !header.is_empty() {
            // If the text file is supposed to have no header and there is one,
            // then return error.
            return Err(NoteError::CannotPrependHeader {
                existing_header: header
                    .lines()
                    .take(5)
                    .map(|s| s.to_string())
                    .collect::<String>(),
            });
        };

        // Deserialize the note's header read from disk.
        // Store the front matter in the context for later use in templates.
        let fm = FrontMatter::try_from_content(&content)?;
        context.insert_front_matter(&fm);

        match template_kind {
            TemplateKind::SyncFilename =>
            // No rendering to markdown is required. `content` is read from disk and left untouched.
            {
                fm.assert_not_empty()?;
                // Check if the compulsory field is present.
                fm.assert_compulsory_field()?;
                Ok(Self {
                    context,
                    content,
                    rendered_filename: PathBuf::new(),
                })
            }
            TemplateKind::None =>
            // No rendering to markdown is required. `content` is read from disk and left untouched.
            // A rendition to HTML may follow.
            {
                fm.assert_not_empty()?;
                // Check if the compulsory field is present.
                fm.assert_compulsory_field()?;
                Ok(Self {
                    context,
                    content,
                    rendered_filename: PathBuf::new(),
                })
            }
            TemplateKind::FromTextFile => Self::from_content_template(context, template_kind),
            // This should not happen. Use `Self::from_content_template()` instead.
            _ => {
                panic!(
                    "Contract violation: `template_kind=={:?}` is not acceptable here.",
                    template_kind
                );
            }
        }
    }

    /// Constructor that creates a new note by filling in the content
    /// template `template`.
    ///
    /// Contract: `template_kind` should be NOT one of:
    /// * `TemplateKind::SyncFilename`,
    /// * `TemplateKind::None`
    ///
    /// Panics if this is the case.
    ///
    pub fn from_content_template(
        mut context: Context,
        template_kind: TemplateKind,
    ) -> Result<Note<T>, NoteError> {
        log::trace!(
            "Available substitution variables for content template:\n{:#?}",
            *context
        );

        log::trace!(
            "Applying content template: {:?}",
            template_kind.get_content_template_name()
        );

        // render template

        let content: T = T::from({
            let mut tera = Tera::default();
            tera.extend(&TERA)?;

            // Panics, if the content template does not exist (see contract).
            // Returns an error, when the rendition goes wrong.
            tera.render_str(&template_kind.get_content_template(), &context)
                .map_err(|e| {
                    note_error_tera_template!(
                        e,
                        template_kind.get_content_template_name().to_string()
                    )
                })?
        });

        log::debug!(
            "Rendered content template:\n---\n{}\n---\n{}",
            content.header(),
            content.body().trim()
        );

        // deserialize the rendered template
        let fm = FrontMatter::try_from_content(&content)?;

        context.insert_front_matter(&fm);

        // Return new note.
        Ok(Note {
            context,
            content,
            rendered_filename: PathBuf::new(),
        })
    }

    /// Applies a Tera template to the notes context in order to generate a
    /// sanitized filename that is in sync with the note's meta data stored in
    /// its front matter.
    pub fn render_filename(&mut self, template_kind: TemplateKind) -> Result<(), NoteError> {
        log::trace!(
            "Available substitution variables for the filename template:\n{:#?}",
            *self.context
        );
        log::trace!(
            "Applying the filename template: {:?}",
            template_kind.get_filename_template_name()
        );

        // render template
        let mut file_path = self.context.dir_path.to_owned();
        let mut tera = Tera::default();
        tera.extend(&TERA)?;

        match tera.render_str(&template_kind.get_filename_template(), &self.context) {
            Ok(filename) => {
                log::debug!("Rendered filename template:\n{:?}", filename.trim());
                file_path.push(filename.trim());
            }
            Err(e) => {
                return Err(note_error_tera_template!(
                    e,
                    template_kind.get_filename_template_name().to_string()
                ));
            }
        }

        file_path.shorten_filename();
        self.rendered_filename = file_path;
        Ok(())
    }

    /// Checks if `self.rendered_filename` is taken already.
    /// If yes, some copy counter is appended/incremented.
    /// Contract: `render_filename` must have been executed before.
    pub fn set_next_unused_rendered_filename(&mut self) -> Result<(), NoteError> {
        debug_assert_ne!(self.rendered_filename, PathBuf::new());

        self.rendered_filename.set_next_unused()?;
        Ok(())
    }

    /// Checks if `alt_path` is equal to `self.rendered_filename`
    /// without considering their copy counter.
    /// If they are similar, `self.rendered_filename` becomes `alt_path`.
    /// If they are different, then we continue incrementing the copy
    /// counter in `self.rendered_filename` until we find a free spot.
    /// (Same as in `set_next_unused_rendered_filename()`).
    /// Contract: `render_filename` must have been executed before.
    pub fn set_next_unused_rendered_filename_or(
        &mut self,
        alt_path: &Path,
    ) -> Result<(), NoteError> {
        debug_assert_ne!(self.rendered_filename, PathBuf::new());

        if self.rendered_filename.exclude_copy_counter_eq(alt_path) {
            self.rendered_filename = alt_path.to_path_buf();
        } else {
            self.rendered_filename.set_next_unused()?;
        }
        Ok(())
    }

    /// Writes the note to disk using the note's `content` and the note's
    /// `rendered_filename`.
    pub fn save(&self) -> Result<(), NoteError> {
        debug_assert_ne!(self.rendered_filename, PathBuf::new());

        log::trace!(
            "Writing the note's content to file: {:?}",
            self.rendered_filename
        );
        self.content.save_as(&self.rendered_filename)?;
        Ok(())
    }

    /// Rename the file `from_path` to `self.rendered_filename`.
    /// Silently fails is source and target are identical.
    /// Contract: `render_filename` must have been executed before.
    pub fn rename_file_from(&self, from_path: &Path) -> Result<(), NoteError> {
        debug_assert_ne!(self.rendered_filename, PathBuf::new());

        if !from_path.exclude_copy_counter_eq(&self.rendered_filename) {
            // rename file
            fs::rename(from_path, &self.rendered_filename)?;
            log::trace!("File renamed to {:?}", self.rendered_filename);
        }
        Ok(())
    }

    /// Write the note to disk and remove the file at the previous location.
    /// Similar to `rename_from()`, but the target is replaced by `self.content`.
    /// Silently fails is source and target are identical.
    /// Contract: `render_filename` must have been executed before.
    pub fn save_and_delete_from(&mut self, from_path: &Path) -> Result<(), NoteError> {
        debug_assert_ne!(self.rendered_filename, PathBuf::new());

        self.save()?;
        if from_path != self.rendered_filename {
            log::trace!("Deleting file: {:?}", from_path);
            fs::remove_file(from_path)?;
        }
        Ok(())
    }

    /// Renders `self` into HTML and saves the result in `export_dir`. If
    /// `export_dir` is the empty string, the directory of `note_path` is
    /// used. `-` dumps the rendition to STDOUT.
    /// This function reads `self.rendered_filename` or - if empty -
    /// `self.context.path` is used to determine the filename of the
    /// html rendition.
    pub fn export_html(
        &self,
        html_template: &str,
        export_dir: &Path,
        local_link_kind: LocalLinkKind,
    ) -> Result<(), NoteError> {
        // Determine filename of html-file.
        let mut html_path = PathBuf::new();
        let current_path = if self.rendered_filename != PathBuf::new() {
            &self.rendered_filename
        } else {
            &self.context.path
        };

        let current_dir_path = current_path.parent().unwrap_or_else(|| Path::new(""));

        if export_dir
            .as_os_str()
            .to_str()
            .unwrap_or_default()
            .is_empty()
        {
            html_path = current_path
                .parent()
                .unwrap_or_else(|| Path::new(""))
                .to_path_buf();
            let mut html_filename = current_path
                .file_name()
                .unwrap_or_default()
                .to_str()
                .unwrap_or_default()
                .to_string();
            html_filename.push_str(".html");
            html_path.push(PathBuf::from(html_filename.as_str()));
        } else if export_dir.display().to_string() != "-" {
            html_path = export_dir.to_owned();
            let mut html_filename = current_path
                .file_name()
                .unwrap_or_default()
                .to_str()
                .unwrap_or_default()
                .to_string();
            html_filename.push_str(HTML_EXT);
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
        let note_path_ext = current_path
            .extension()
            .unwrap_or_default()
            .to_str()
            .unwrap_or_default()
            .to_string();

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
            handle.write_all(
                self.render_content_to_html(&note_path_ext, html_template)
                    .map(|html| {
                        rewrite_links(
                            html,
                            &self.context.root_path,
                            current_dir_path,
                            local_link_kind,
                            // Do append `.html` to `.md` in links.
                            true,
                            Arc::new(RwLock::new(HashSet::new())),
                        )
                    })?
                    .as_bytes(),
            )?;
        } else {
            let mut handle = OpenOptions::new()
                .write(true)
                .create(true)
                .open(&html_path)?;
            // Write HTML rendition.
            handle.write_all(
                self.render_content_to_html(&note_path_ext, html_template)
                    .map(|html| {
                        rewrite_links(
                            html,
                            &self.context.root_path,
                            current_dir_path,
                            local_link_kind,
                            // Do append `.html` to `.md` in links.
                            true,
                            Arc::new(RwLock::new(HashSet::new())),
                        )
                    })?
                    .as_bytes(),
            )?;
        };
        Ok(())
    }

    #[inline]
    /// First, determines the markup language from the file extension or
    /// the `fm_file_ext` YAML variable, if present.
    /// Then calls the appropriate markup renderer.
    /// Finally the result is rendered with the `HTML_VIEWER_TMPL`
    /// template. This template expects the template variable
    /// `TMPL_HTML_VAR_NOTE_JS` in `self.context` to be set.
    pub fn render_content_to_html(
        &self,
        // We need the file extension to determine the
        // markup language.
        file_ext: &str,
        // HTML template for this rendition.
        tmpl: &str,
    ) -> Result<String, NoteError> {
        // Deserialize.

        // Render Body.
        let input = self.content.body();

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

        let mut html_context = self.context.clone();

        // Register rendered body.
        html_context.insert(TMPL_HTML_VAR_NOTE_BODY_HTML, &html_output);

        // Insert the raw CSS
        html_context.insert(
            TMPL_HTML_VAR_NOTE_CSS,
            &LIB_CFG.read().unwrap().tmpl_html.css,
        );
        // Insert the web server path to get the CSS loaded.
        html_context.insert(
            TMPL_HTML_VAR_NOTE_CSS_PATH,
            TMPL_HTML_VAR_NOTE_CSS_PATH_VALUE,
        );

        let mut tera = Tera::default();
        tera.extend(&TERA)?;
        let html = tera.render_str(tmpl, &html_context).map_err(|e| {
            note_error_tera_template!(e, "[html_tmpl] viewer/exporter_tmpl ".to_string())
        })?;

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
        let parser = SyntaxPreprocessor::new(parser);

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

        let expected_front_matter = FrontMatter(expected);

        assert_eq!(expected_front_matter, FrontMatter::try_from(input).unwrap());

        //
        // Is empty.
        let input = "";

        assert!(FrontMatter::try_from(input).is_ok());

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
        let input2 = FrontMatter(tmp);

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

    #[test]
    fn test_from_text_file1() {
        //
        // Example with `TemplateKind::SyncFilename`
        //
        use crate::content::Content;
        use crate::content::ContentString;
        use crate::context::Context;
        use crate::note::Note;
        use crate::template::TemplateKind;
        use std::env::temp_dir;
        use std::fs;
        // Prepare test: create existing note.
        let raw = r#"---
title: "My day"
subtitle: "Note"
---
Body text
"#;
        let notefile = temp_dir().join("20221031-hello.md");
        fs::write(&notefile, raw.as_bytes()).unwrap();
        let expected = temp_dir().join("20221031-My day--Note.md");
        let _ = fs::remove_file(&expected);
        // Start test.
        let context = Context::from(&notefile);
        // Create note object.
        let content = <ContentString as Content>::open(&notefile).unwrap();
        // You can plug in your own type (must impl. `Content`).
        let mut n = Note::from_text_file(context, content, TemplateKind::SyncFilename).unwrap();
        n.render_filename(TemplateKind::SyncFilename).unwrap();
        n.set_next_unused_rendered_filename_or(&n.context.path.clone())
            .unwrap();
        assert_eq!(n.rendered_filename, expected);
        // Rename file on the disk.
        n.rename_file_from(&n.context.path).unwrap();
        assert!(n.rendered_filename.is_file());
    }

    #[test]
    fn test_from_text_file2() {
        // Example with `TemplateKind::None`
        //
        //    This constructor is called, when `Note` is solely created for
        // HTML rendering and no templates will be applied.
        //
        use crate::config::LIB_CFG;
        use crate::config::TMPL_HTML_VAR_NOTE_JS;
        use crate::content::Content;
        use crate::content::ContentString;
        use crate::context::Context;
        use crate::note::Note;
        use crate::template::TemplateKind;
        use std::env::temp_dir;
        use std::fs;
        // Prepare test: create existing note file.
        let raw = r#"---
title: "My day"
subtitle: "Note"
---
Body text
"#;
        let notefile = temp_dir().join("20221030-My day--Note.md");
        fs::write(&notefile, raw.as_bytes()).unwrap();
        // Start test
        // Only minimal context is needed, because no templates are applied later.
        let mut context = Context::from(&notefile);
        // We do not inject any JavaScript.
        context.insert(TMPL_HTML_VAR_NOTE_JS, &"".to_string());
        // Create note object.
        let content = <ContentString as Content>::open(&notefile).unwrap();
        // You can plug in your own type (must impl. `Content`).
        let n = Note::from_text_file(context, content, TemplateKind::None).unwrap();
        // Check the HTML rendition.
        let html = n
            .render_content_to_html("md", &LIB_CFG.read().unwrap().tmpl_html.viewer)
            .unwrap();
        assert!(html.starts_with("<!DOCTYPE html>\n<html"))
    }

    #[test]
    fn test_from_text_file3() {
        //
        // Example with `TemplateKind::FromTextFile`
        //
        use crate::content::Content;
        use crate::content::ContentString;
        use crate::context::Context;
        use crate::note::Note;
        use crate::template::TemplateKind;
        use std::env::temp_dir;
        use std::fs;

        // Prepare test: create existing note file without header.
        let raw = "Body text without header";
        let notefile = temp_dir().join("20221030-hello -- world.md");
        let _ = fs::write(&notefile, raw.as_bytes());
        let expected = temp_dir().join("20221030-hello--world.md");
        let _ = fs::remove_file(&expected);
        // Start test.
        let context = Context::from(&notefile);
        // Create note object.
        let content = <ContentString as Content>::open(&notefile).unwrap();
        // You can plug in your own type (must impl. `Content`).
        let mut n =
            Note::from_text_file(context.clone(), content, TemplateKind::FromTextFile).unwrap();
        assert!(!n.content.header().is_empty());
        assert_eq!(n.context.get("fm_title").unwrap().as_str(), Some("hello "));
        assert_eq!(
            n.context.get("fm_subtitle").unwrap().as_str(),
            Some(" world")
        );
        assert_eq!(n.content.body().trim(), raw);
        n.render_filename(TemplateKind::FromTextFile).unwrap();
        n.set_next_unused_rendered_filename().unwrap();
        n.save_and_delete_from(&context.path).unwrap();

        // Check the new file with header
        assert_eq!(&n.rendered_filename, &expected);
        assert!(n.rendered_filename.is_file());
        let raw_note = fs::read_to_string(n.rendered_filename).unwrap();
        #[cfg(not(target_family = "windows"))]
        assert!(raw_note.starts_with("\u{feff}---\ntitle:      \"hello \""));
        #[cfg(target_family = "windows")]
        assert!(raw_note.starts_with("\u{feff}---\r\ntitle:      \"hello \""));
    }

    #[test]
    fn test_from_content_template1() {
        // Example with `TemplateKind::New`
        //
        use crate::content::Content;
        use crate::content::ContentString;
        use crate::context::Context;
        use crate::note::Note;
        use crate::template::TemplateKind;
        use std::env::temp_dir;
        use std::fs;

        // Create a directory for the new note.
        let notedir = temp_dir().join("123-my dir/");
        fs::create_dir_all(&notedir).unwrap();

        // Store the path in `context`.
        let context = Context::from(&notedir);

        // Create the `Note` object.
        // You can plug in your own type (must impl. `Content`).
        let mut n: Note<ContentString> =
            Note::from_content_template(context, TemplateKind::New).unwrap();
        assert!(n.content.header().starts_with("title:      \"my dir\""));
        assert_eq!(n.content.borrow_dependent().body, "\n\n");

        // Check the title and subtitle in the note's header.
        assert_eq!(n.context.get("fm_title").unwrap().as_str(), Some("my dir"));
        assert_eq!(n.context.get("fm_subtitle").unwrap().as_str(), Some("Note"));
        n.render_filename(TemplateKind::New).unwrap();
        n.set_next_unused_rendered_filename().unwrap();
        n.save().unwrap();

        // Check the created new note file.
        assert!(n.rendered_filename.is_file());
        let raw_note = fs::read_to_string(n.rendered_filename).unwrap();
        #[cfg(not(target_family = "windows"))]
        assert!(raw_note.starts_with("\u{feff}---\ntitle:      \"my dir\""));
        #[cfg(target_family = "windows")]
        assert!(raw_note.starts_with("\u{feff}---\r\ntitle:      \"my dir\""));
    }

    #[test]
    fn test_from_content_template2() {
        // Example with `TemplateKind::FromClipboard`

        use crate::config::{TMPL_VAR_CLIPBOARD, TMPL_VAR_CLIPBOARD_HEADER};
        use crate::config::{TMPL_VAR_STDIN, TMPL_VAR_STDIN_HEADER};
        use crate::content::Content;
        use crate::content::ContentString;
        use crate::context::Context;
        use crate::note::Note;
        use crate::template::TemplateKind;
        use std::env::temp_dir;
        use std::fs;

        // Directory for the new note.
        let notedir = temp_dir();

        // Store the path in `context`.
        let mut context = Context::from(&notedir);
        let clipboard = ContentString::from("clp\n".to_string());
        context
            .insert_content(TMPL_VAR_CLIPBOARD, TMPL_VAR_CLIPBOARD_HEADER, &clipboard)
            .unwrap();
        let stdin = ContentString::from("std\n".to_string());
        context
            .insert_content(TMPL_VAR_STDIN, TMPL_VAR_STDIN_HEADER, &stdin)
            .unwrap();
        // This is the condition to choose: `TemplateKind::FromClipboard`:
        assert!(clipboard.header().is_empty() && stdin.header().is_empty());
        assert!(!clipboard.body().is_empty() || !stdin.body().is_empty());

        // Create the `Note` object.
        // You can plug in your own type (must impl. `Content`).
        let mut n: Note<ContentString> =
            Note::from_content_template(context, TemplateKind::FromClipboard).unwrap();
        let expected_body = "\nstd\nclp\n\n\n";
        assert_eq!(n.content.body(), expected_body);
        // Check the title and subtitle in the note's header.
        assert_eq!(
            n.context.get("fm_title").unwrap().as_str(),
            Some("std\nclp\n")
        );

        assert_eq!(n.context.get("fm_subtitle").unwrap().as_str(), Some("Note"));
        n.render_filename(TemplateKind::FromClipboard).unwrap();
        n.set_next_unused_rendered_filename().unwrap();
        n.save().unwrap();

        // Check the new note file.
        assert!(n
            .rendered_filename
            .as_os_str()
            .to_str()
            .unwrap()
            .contains("std-clp--Note"));
        assert!(n.rendered_filename.is_file());
        let raw_note = fs::read_to_string(&n.rendered_filename).unwrap();
        #[cfg(not(target_family = "windows"))]
        assert!(raw_note.starts_with("\u{feff}---\ntitle:      \"std\\nclp\\n\""));
        #[cfg(target_family = "windows")]
        assert!(raw_note.starts_with("\u{feff}---\r\ntitle:      \"std"));
    }

    #[test]
    fn test_from_content_template3() {
        // Example with `TemplateKind::FromClipboardYaml`

        use crate::config::{TMPL_VAR_CLIPBOARD, TMPL_VAR_CLIPBOARD_HEADER};
        use crate::config::{TMPL_VAR_STDIN, TMPL_VAR_STDIN_HEADER};
        use crate::content::Content;
        use crate::content::ContentString;
        use crate::context::Context;
        use crate::filter::CUT_LEN_MAX;
        use crate::note::Note;
        use crate::template::TemplateKind;
        use std::env::temp_dir;
        use std::fs;

        // Prepare test.
        // Directory for the new note.
        let notedir = temp_dir().join("123-my dir/");

        // Run test.
        // Store the path in `context`.
        let mut context = Context::from(&notedir);
        let clipboard = ContentString::from("my clipboard\n".to_string());
        context
            .insert_content(TMPL_VAR_CLIPBOARD, TMPL_VAR_CLIPBOARD_HEADER, &clipboard)
            .unwrap();
        let stdin =
            ContentString::from("---\nsubtitle: \"this overwrites\"\n---\nstdin body".to_string());
        context
            .insert_content(TMPL_VAR_STDIN, TMPL_VAR_STDIN_HEADER, &stdin)
            .unwrap();
        // This is the condition to choose: `TemplateKind::FromClipboardYaml`:
        assert!(!clipboard.header().is_empty() || !stdin.header().is_empty());

        // Create the `Note` object.
        // You can plug in your own type (must impl. `Content`).
        let mut n: Note<ContentString> =
            Note::from_content_template(context, TemplateKind::FromClipboardYaml).unwrap();
        let expected_body = "\nstdin bodymy clipboard\n\n\n";
        assert_eq!(n.content.body(), expected_body);
        // Check the title and subtitle in the note's header.
        assert_eq!(n.context.get("fm_title").unwrap().as_str(), Some("my dir"));

        assert_eq!(
            n.context.get("fm_subtitle").unwrap().as_str(),
            // Remember: in debug titles are very short. The code only works,
            // because the string is pure ASCII (not UTF-8).
            Some(&"this overwrites"[..CUT_LEN_MAX - 1])
        );
        n.render_filename(TemplateKind::FromClipboardYaml).unwrap();
        n.set_next_unused_rendered_filename().unwrap();
        n.save().unwrap();

        // Check the new note file.
        assert!(n
            .rendered_filename
            .as_os_str()
            .to_str()
            .unwrap()
            .contains(&"my dir--this overwrites"[..CUT_LEN_MAX - 1]));
        assert!(n.rendered_filename.is_file());
        let raw_note = fs::read_to_string(n.rendered_filename).unwrap();
        #[cfg(not(target_family = "windows"))]
        assert!(raw_note.starts_with("\u{feff}---\ntitle:      \"my dir\""));
        #[cfg(target_family = "windows")]
        assert!(raw_note.starts_with("\u{feff}---\r\ntitle:      \"my dir\""));
    }

    #[test]
    fn test_from_content_template4() {
        // Example with `TemplateKind::AnnotateFile`

        use crate::config::FILENAME_EXTENSION_DEFAULT;
        use crate::config::{TMPL_VAR_CLIPBOARD, TMPL_VAR_CLIPBOARD_HEADER};
        use crate::config::{TMPL_VAR_STDIN, TMPL_VAR_STDIN_HEADER};
        use crate::content::Content;
        use crate::content::ContentString;
        use crate::context::Context;
        use crate::note::Note;
        use crate::template::TemplateKind;
        use std::env::temp_dir;
        use std::fs;

        // Prepare the test.
        // Create some non-Tp-Note-file.
        let raw = "This simulates a non tp-note file";
        let non_notefile = temp_dir().join("20221030-some.pdf");
        fs::write(&non_notefile, raw.as_bytes()).unwrap();
        let mut expected = temp_dir().join("20221030-some.pdf--Note.md");
        // In case this test runs on Windows, the extension may be different.
        expected.set_extension(FILENAME_EXTENSION_DEFAULT);
        let _ = fs::remove_file(&expected);

        // Run the test.
        // Store the path in `context`.
        let mut context = Context::from(&non_notefile);
        let clipboard = ContentString::from_string_with_cr("my clipboard\n".to_string());
        context
            .insert_content(TMPL_VAR_CLIPBOARD, TMPL_VAR_CLIPBOARD_HEADER, &clipboard)
            .unwrap();
        let stdin = ContentString::from_string_with_cr("my stdin\n".to_string());
        context
            .insert_content(TMPL_VAR_STDIN, TMPL_VAR_STDIN_HEADER, &stdin)
            .unwrap();

        // Create the `Note` object.
        // You can plug in your own type (must impl. `Content`).
        let mut n: Note<ContentString> =
            Note::from_content_template(context, TemplateKind::AnnotateFile).unwrap();
        let expected_body =
            "\n[20221030-some.pdf](<20221030-some.pdf>)\n\nmy stdin\nmy clipboard\n\n\n";
        assert_eq!(n.content.body(), expected_body);
        // Check the title and subtitle in the note's header.
        assert_eq!(
            n.context.get("fm_title").unwrap().as_str(),
            Some("some.pdf")
        );
        assert_eq!(n.context.get("fm_subtitle").unwrap().as_str(), Some("Note"));

        n.render_filename(TemplateKind::AnnotateFile).unwrap();
        n.set_next_unused_rendered_filename().unwrap();
        n.save().unwrap();

        // Check the new note file.
        assert_eq!(n.rendered_filename, expected);
        assert!(n.rendered_filename.is_file());
    }
}
