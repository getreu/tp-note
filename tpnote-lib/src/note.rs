//! Tp-Note's low level API, creating a memory representation of a
//! note file by inserting Tp-Note's
//! environment data in some templates. If the note exists on disk already,
//! the memory representation is established be reading the note file and
//! parsing its front matter.
//! NB: The high level API is in the module `tpnote_lib::workflow`.

use crate::config::LIB_CFG;
use crate::config::TMPL_HTML_VAR_EXPORTER_DOC_CSS;
use crate::config::TMPL_HTML_VAR_EXPORTER_HIGHLIGHTING_CSS;
use crate::config::TMPL_HTML_VAR_VIEWER_DOC_CSS_PATH;
use crate::config::TMPL_HTML_VAR_VIEWER_DOC_CSS_PATH_VALUE;
use crate::config::TMPL_HTML_VAR_VIEWER_DOC_JS;
use crate::config::TMPL_HTML_VAR_VIEWER_HIGHLIGHTING_CSS_PATH;
use crate::config::TMPL_HTML_VAR_VIEWER_HIGHLIGHTING_CSS_PATH_VALUE;
use crate::config::TMPL_VAR_DOC;
use crate::config::TMPL_VAR_DOC_HEADER;
use crate::content::Content;
use crate::context::Context;
use crate::context::HasSettings;
use crate::context::ReadyForTemplate;
use crate::error::NoteError;
use crate::filename::NotePath;
use crate::filename::NotePathBuf;
use crate::filter::TERA;
use crate::front_matter::FrontMatter;
use crate::note_error_tera_template;
use crate::template::TemplateKind;
use std::default::Default;
use std::fs;
use std::path::{Path, PathBuf};
use std::str;
use tera::Tera;

/// This constant is used by Tera as template name for `tera::render_str()`.
/// Unfortunately it is private there, this is why we must redefine it here.
pub(crate) const ONE_OFF_TEMPLATE_NAME: &str = "__tera_one_off";

//#[derive(Debug, PartialEq)]
/// Represents a note.
/// 1. The `ContentString`'s header is deserialized into `FrontMatter`.
/// 2. `FrontMatter` is stored in `Context` with some environment data.
/// 3. `Context` data is filled in some filename template.
/// 4. The result is stored in `rendered_filename`.
pub struct Note<T: Content> {
    /// Captured environment of _Tp-Note_ that
    /// is used to fill in templates.
    pub context: Context<ReadyForTemplate>,
    /// The full text content of the note, including
    /// its front matter.
    pub content: T,
    /// This field equals to `PathBuf::new()` until `self.render_filename()`
    /// is called.
    pub rendered_filename: PathBuf,
}

impl<T: Content> Note<T> {
    /// Constructor creating a `Note` memory representation of some content
    /// that exists outside of Tp-Note provided by the `content` object. No file
    /// content is read from disk.
    ///
    /// In case of `TemplateKind::SyncFilename` the `content` is required to
    /// have a header with front matter.
    ///
    /// Contract: `template_kind` must be one of:
    /// * `TemplateKind::SyncFilename`,
    /// * `TemplateKind::None` or
    /// * `TemplateKind::FromTextFile`.
    ///
    /// Panics otherwise. Use `Note::from_content_template()` in those cases.
    ///
    pub fn from_existing_content(
        context: Context<HasSettings>,
        content: T,
        template_kind: TemplateKind,
    ) -> Result<Note<T>, NoteError> {
        match template_kind {
            // No rendering to markdown is required. `content` is read from disk and left untouched.
            TemplateKind::SyncFilename => {
                // Deserialize the note's header read from disk.
                // Store the front matter in the context for later use in templates.
                let fm = FrontMatter::try_from(content.header())?;
                let context = context.insert_front_matter(&fm);
                let context = context.set_state_ready_for_template();
                context.assert_precoditions()?;
                Ok(Note {
                    context,
                    content,
                    rendered_filename: PathBuf::new(),
                })
            }
            // No rendering to markdown is required. `content` is read from disk and left untouched.
            // A rendition to HTML may follow.
            TemplateKind::None => {
                let context = context.set_state_ready_for_template();
                context.assert_precoditions()?;
                Ok(Note {
                    context,
                    content,
                    rendered_filename: PathBuf::new(),
                })
            }
            TemplateKind::FromTextFile => {
                // This is part of the contract for this template:
                debug_assert!(content.header().is_empty());
                debug_assert!(!content.body().is_empty());
                let mut context = context;
                // This template expects the key `TMPL_VAR_DOC` which is
                // inserted with `content`.
                context.insert_front_matter_and_raw_text_from_existing_content(&vec![&content])?;
                Note::from_content_template(
                    context.set_state_ready_for_template(),
                    TemplateKind::FromTextFile,
                )
            }
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
    /// template `template` with the data read from `context`.
    /// The result is an initialized `self.content`.
    ///
    /// Contract: `template_kind` should be NOT one of:
    /// * `TemplateKind::SyncFilename`,
    /// * `TemplateKind::None`
    ///
    /// Panics if this is the case.
    ///
    pub fn from_content_template(
        context: Context<ReadyForTemplate>,
        template_kind: TemplateKind,
    ) -> Result<Note<T>, NoteError> {
        // Add content to context.
        log::trace!(
            "Available substitution variables for the content template:\n{:#?}",
            *context
        );

        debug_assert!(match template_kind {
            TemplateKind::SyncFilename => panic!("`TemplateKind::SyncFilename` not allowed here"),

            TemplateKind::None => panic!("`TemplateKind::None` not allowed here"),
            _ => true,
        });

        // Render template
        let new_content: T = T::from_string(
            {
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
            },
            TMPL_VAR_DOC_HEADER.to_string(),
            TMPL_VAR_DOC.to_string(),
        );

        log::debug!(
            "Rendered content template:\n---\n{}\n---\n{}",
            new_content.header(),
            new_content.body().trim()
        );

        // Deserialize the rendered template
        let fm = FrontMatter::try_from(new_content.header())?;

        let new_context = Context::from_context_path(&context)
            .insert_front_matter(&fm)
            .set_state_ready_for_template();

        // Return new note.
        Ok(Note {
            context: new_context,
            content: new_content,
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
        // Render template
        let mut file_path = self.context.dir_path.to_owned();
        let mut tera = Tera::default();
        tera.extend(&TERA)?;

        match tera.render_str(&template_kind.get_filename_template(), &self.context) {
            Ok(filename) => {
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
            // Rename file
            fs::rename(from_path, &self.rendered_filename)?;
            log::trace!(
                "File renamed to {}",
                self.rendered_filename.to_str().unwrap_or_default()
            );
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

    #[inline]
    /// Renders `self.content` to HTML by calling the appropriate markup
    /// renderer. The `html_tmpl` injects JavaScript code with the
    /// key `TMPL_HTML_VAR_VIEWER_DOC_JS`. This code is provided with the key
    /// `TMPL_HTML_VAR_VIEWER_DOC_JS` in `self.context.ct` and must to be set.
    ///
    /// Contract:
    /// * `self.context` is in a valid `HasSettings` state. All other keys
    ///    except `TMPL_HTML_VAR_VIEWER_DOC_JS` are ignored.
    /// * `self.content.body_name == TMPL_VAR_DOC`. The HTML template expects
    ///   this name.
    /// * The `html_tmpl` expects `content` to have a header with at least one
    ///   `title:` field.
    ///
    pub fn render_content_to_html(
        &self,
        // HTML template for this rendition.
        tmpl: &str,
    ) -> Result<String, NoteError> {
        self.context.debug_assert_paths_and_map_in_sync();

        let mut html_context = Context::from_context_path(&self.context);

        // Copy `TMPL_HTML_VAR_VIEWER_DOC_JS`
        if let Some(val) = self.context.get(TMPL_HTML_VAR_VIEWER_DOC_JS) {
            html_context.insert(TMPL_HTML_VAR_VIEWER_DOC_JS, val);
        }

        html_context
            .insert_front_matter_and_raw_text_from_existing_content(&vec![&self.content])?;

        {
            let lib_cfg = &LIB_CFG.read_recursive();

            // Insert the raw CSS
            html_context.insert(
                TMPL_HTML_VAR_EXPORTER_DOC_CSS,
                &(lib_cfg.tmpl_html.exporter_doc_css),
            );

            // Insert the raw CSS
            #[cfg(feature = "renderer")]
            html_context.insert(
                TMPL_HTML_VAR_EXPORTER_HIGHLIGHTING_CSS,
                &(lib_cfg.tmpl_html.exporter_highlighting_css),
            );
        } // Drop `lib_cfg`.

        // Insert the raw CSS
        #[cfg(not(feature = "renderer"))]
        html_context.insert(TMPL_HTML_VAR_EXPORTER_HIGHLIGHTING_CSS, "");

        // Insert the web server path to get the Tp-Note's CSS loaded.
        html_context.insert(
            TMPL_HTML_VAR_VIEWER_DOC_CSS_PATH,
            TMPL_HTML_VAR_VIEWER_DOC_CSS_PATH_VALUE,
        );

        // Insert the web server path to get the highlighting CSS loaded.
        html_context.insert(
            TMPL_HTML_VAR_VIEWER_HIGHLIGHTING_CSS_PATH,
            TMPL_HTML_VAR_VIEWER_HIGHLIGHTING_CSS_PATH_VALUE,
        );

        log::trace!(
            "Available substitution variables for the HTML template:\
            \n{:#?}",
            html_context
        );

        let mut tera = Tera::default();
        tera.extend(&TERA)?;
        // Switch `autoescape_on()` only for HTML templates.
        tera.autoescape_on(vec![ONE_OFF_TEMPLATE_NAME]);
        let html = tera.render_str(tmpl, &html_context).map_err(|e| {
            note_error_tera_template!(e, "[html_tmpl] viewer/exporter_tmpl ".to_string())
        })?;

        Ok(html)
    }
}

#[cfg(test)]
mod tests {
    use super::Context;
    use super::FrontMatter;
    use crate::config::TMPL_VAR_FM_ALL;
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

        //panic!("{:?}", &expected_front_matter);
        assert_eq!(expected_front_matter, FrontMatter::try_from(input).unwrap());
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
        let mut tmp2 = tera::Map::new();
        tmp2.insert("fm_file_ext".to_string(), Value::String("md".to_string())); // String
        tmp2.insert("fm_height".to_string(), json!(1.23)); // Number()
        tmp2.insert("fm_count".to_string(), json!(2)); // Number()
        tmp2.insert("fm_neg".to_string(), json!(-1)); // Number()
        tmp2.insert("fm_flag".to_string(), json!(true)); // Bool()
        tmp2.insert("fm_numbers".to_string(), json!([1, 3, 5])); // Array([Numbers()..])!

        let input1 = Context::from(Path::new("a/b/test.md")).unwrap();
        let input2 = FrontMatter(tmp);

        let mut expected = Context::from(Path::new("a/b/test.md")).unwrap();
        tmp2.remove("fm_numbers");
        tmp2.insert("fm_numbers".to_string(), json!([1, 3, 5])); // String()!
        (*expected).insert(TMPL_VAR_FM_ALL.to_string(), &tmp2); // Map()
        let expected = expected.set_state_ready_for_template();

        let result = input1.insert_front_matter(&input2);
        let result = result.set_state_ready_for_template();

        assert_eq!(result, expected);
    }

    #[test]
    fn test_from_existing_content1() {
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
        let context = Context::from(&notefile).unwrap();
        // Create note object.
        let content = <ContentString as Content>::open(&notefile).unwrap();
        // You can plug in your own type (must impl. `Content`).
        let mut n = Note::<ContentString>::from_existing_content(
            context,
            content,
            TemplateKind::SyncFilename,
        )
        .unwrap();
        n.render_filename(TemplateKind::SyncFilename).unwrap();
        n.set_next_unused_rendered_filename_or(&n.context.path.clone())
            .unwrap();
        assert_eq!(n.rendered_filename, expected);
        // Rename file on the disk.
        n.rename_file_from(&n.context.path).unwrap();
        assert!(n.rendered_filename.is_file());
    }

    #[test]
    fn test_from_existing_content2() {
        // Example with `TemplateKind::None`
        //
        //    This constructor is called, when `Note` is solely created for
        // HTML rendering and no templates will be applied.
        //
        use crate::config::LIB_CFG;
        use crate::config::TMPL_HTML_VAR_VIEWER_DOC_JS;
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
        let mut context = Context::from(&notefile).unwrap();
        // We do not inject any JavaScript.
        context.insert(TMPL_HTML_VAR_VIEWER_DOC_JS, &"".to_string());
        // Create note object.
        let content = <ContentString as Content>::open(&notefile).unwrap();
        // You can plug in your own type (must impl. `Content`).
        let n: Note<ContentString> =
            Note::<ContentString>::from_existing_content(context, content, TemplateKind::None)
                .unwrap();
        // Check the HTML rendition.
        let html = n
            .render_content_to_html(&LIB_CFG.read_recursive().tmpl_html.viewer)
            .unwrap();
        assert!(html.starts_with("<!DOCTYPE html>\n<html"))
    }

    #[test]
    fn test_from_existing_content3() {
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
        let context = Context::from(&notefile).unwrap();
        // Create note object.
        let content = <ContentString as Content>::open(&notefile).unwrap();
        // You can plug in your own type (must impl. `Content`).
        let mut n = Note::<ContentString>::from_existing_content(
            context.clone(),
            content,
            TemplateKind::FromTextFile,
        )
        .unwrap();
        assert!(!n.content.header().is_empty());
        assert_eq!(
            n.context
                .get(TMPL_VAR_FM_ALL)
                .unwrap()
                .get("fm_title")
                .unwrap()
                .as_str(),
            Some("hello ")
        );
        assert_eq!(
            n.context
                .get(TMPL_VAR_FM_ALL)
                .unwrap()
                .get("fm_subtitle")
                .unwrap()
                .as_str(),
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
        assert!(raw_note.starts_with("\u{feff}---\ntitle:        'hello '"));
        #[cfg(target_family = "windows")]
        assert!(raw_note.starts_with("\u{feff}---\r\ntitle:        'hello '"));
    }

    #[test]
    fn test_from_content_template1() {
        // Example with `TemplateKind::New`
        //
        use crate::content::Content;
        use crate::content::ContentString;
        use crate::context::Context;
        use crate::note::Note;
        use crate::settings::Settings;
        use crate::settings::SETTINGS;
        use crate::template::TemplateKind;
        use parking_lot::RwLockWriteGuard;
        use std::env::temp_dir;
        use std::fs;

        // Prepare test.
        let mut settings = SETTINGS.write();
        *settings = Settings::default();
        // This locks `SETTINGS` for further write access in this scope.
        let _settings = RwLockWriteGuard::<'_, _>::downgrade(settings);
        // Create a directory for the new note.
        let notedir = temp_dir().join("123-my dir/");
        fs::create_dir_all(&notedir).unwrap();

        // Store the path in `context`.
        let context = Context::from(&notedir).unwrap();
        //
        let context = context.set_state_ready_for_template();

        // Create the `Note` object.
        // You can plug in your own type (must impl. `Content`).
        let mut n =
            Note::<ContentString>::from_content_template(context, TemplateKind::FromDir).unwrap();
        assert!(n.content.header().starts_with("title:        my dir"));
        assert_eq!(n.content.borrow_dependent().body, "\n\n");

        // Check the title and subtitle in the note's header.
        assert_eq!(
            n.context
                .get(TMPL_VAR_FM_ALL)
                .unwrap()
                .get("fm_title")
                .unwrap()
                .as_str(),
            Some("my dir")
        );
        assert_eq!(
            n.context
                .get(TMPL_VAR_FM_ALL)
                .unwrap()
                .get("fm_subtitle")
                .unwrap()
                .as_str(),
            Some("Note")
        );
        n.render_filename(TemplateKind::FromDir).unwrap();
        n.set_next_unused_rendered_filename().unwrap();
        n.save().unwrap();

        // Check the created new note file.
        assert!(n.rendered_filename.is_file());
        let raw_note = fs::read_to_string(n.rendered_filename).unwrap();
        #[cfg(not(target_family = "windows"))]
        assert!(raw_note.starts_with("\u{feff}---\ntitle:        my dir"));
        #[cfg(target_family = "windows")]
        assert!(raw_note.starts_with("\u{feff}---\r\ntitle:        my dir"));
    }

    #[test]
    fn test_from_content_template2() {
        // Example with `TemplateKind::FromClipboard`

        use crate::content::Content;
        use crate::content::ContentString;
        use crate::context::Context;
        use crate::note::Note;
        use crate::settings::Settings;
        use crate::settings::SETTINGS;
        use crate::template::TemplateKind;
        use parking_lot::RwLockWriteGuard;
        use std::env::temp_dir;
        use std::fs;

        // Prepare test.
        let mut settings = SETTINGS.write();
        *settings = Settings::default();
        // This locks `SETTINGS` for further write access in this scope.
        let _settings = RwLockWriteGuard::<'_, _>::downgrade(settings);
        // Directory for the new note.
        let notedir = temp_dir();

        // Store the path in `context`.
        let mut context = Context::from(&notedir).unwrap();
        let html_clipboard = ContentString::from_string(
            "html_clp\n".to_string(),
            "html_clipboard_header".to_string(),
            "html_clipboard".to_string(),
        );
        let txt_clipboard = ContentString::from_string(
            "txt_clp\n".to_string(),
            "txt_clipboard_header".to_string(),
            "txt_clipboard".to_string(),
        );
        let stdin = ContentString::from_string(
            "std\n".to_string(),
            "stdin_header".to_string(),
            "stdin".to_string(),
        );
        let v = vec![&html_clipboard, &txt_clipboard, &stdin];
        context
            .insert_front_matter_and_raw_text_from_existing_content(&v)
            .unwrap();

        // This is the condition to choose: `TemplateKind::FromClipboard`:
        assert!(
            html_clipboard.header().is_empty()
                && txt_clipboard.header().is_empty()
                && stdin.header().is_empty()
        );
        assert!(
            !html_clipboard.body().is_empty()
                && !txt_clipboard.body().is_empty()
                && !stdin.body().is_empty()
        );

        let context = context.set_state_ready_for_template();

        // Create the `Note` object.
        // You can plug in your own type (must impl. `Content`).
        let mut n =
            Note::<ContentString>::from_content_template(context, TemplateKind::FromClipboard)
                .unwrap();
        let expected_body = "\nstd\ntxt_clp\n\n";
        assert_eq!(n.content.body(), expected_body);
        // Check the title and subtitle in the note's header.
        assert_eq!(
            n.context
                .get(TMPL_VAR_FM_ALL)
                .unwrap()
                .get("fm_title")
                .unwrap()
                .as_str(),
            Some("std")
        );

        assert_eq!(
            n.context
                .get(TMPL_VAR_FM_ALL)
                .unwrap()
                .get("fm_subtitle")
                .unwrap()
                .as_str(),
            Some("Note")
        );
        n.render_filename(TemplateKind::FromClipboard).unwrap();
        n.set_next_unused_rendered_filename().unwrap();
        n.save().unwrap();

        // Check the new note file.
        // println!("{:?}", n.rendered_filename);
        assert!(n
            .rendered_filename
            .as_os_str()
            .to_str()
            .unwrap()
            .contains("std--Note"));
        assert!(n.rendered_filename.is_file());
        let raw_note = fs::read_to_string(&n.rendered_filename).unwrap();
        println!("{}", raw_note);
        #[cfg(not(target_family = "windows"))]
        assert!(raw_note.starts_with("\u{feff}---\ntitle:        std"));
        #[cfg(target_family = "windows")]
        assert!(raw_note.starts_with("\u{feff}---\r\ntitle:"));
    }

    #[test]
    fn test_from_content_template3() {
        // Example with `TemplateKind::FromClipboardYaml`

        use crate::content::Content;
        use crate::content::ContentString;
        use crate::context::Context;
        use crate::filter::TRUNC_LEN_MAX;
        use crate::note::Note;
        use crate::settings::Settings;
        use crate::settings::SETTINGS;
        use crate::template::TemplateKind;
        use parking_lot::RwLockWriteGuard;
        use std::env::temp_dir;
        use std::fs;

        // Prepare test.
        let mut settings = SETTINGS.write();
        *settings = Settings::default();
        // This locks `SETTINGS` for further write access in this scope.
        let _settings = RwLockWriteGuard::<'_, _>::downgrade(settings);
        // Directory for the new note.
        let notedir = temp_dir().join("123-my dir/");

        // Run test.
        // Store the path in `context`.
        let mut context = Context::from(&notedir).unwrap();
        let html_clipboard = ContentString::from_string(
            "my HTML clipboard\n".to_string(),
            "html_clipboard_header".to_string(),
            "html_clipboard".to_string(),
        );
        let txt_clipboard = ContentString::from_string(
            "my TXT clipboard\n".to_string(),
            "txt_clipboard_header".to_string(),
            "txt_clipboard".to_string(),
        );
        let stdin = ContentString::from_string(
            "---\nsubtitle: \"this overwrites\"\n---\nstdin body".to_string(),
            "stdin_header".to_string(),
            "stdin".to_string(),
        );
        let v = vec![&html_clipboard, &txt_clipboard, &stdin];

        context
            .insert_front_matter_and_raw_text_from_existing_content(&v)
            .unwrap();
        // This is the condition to choose: `TemplateKind::FromClipboardYaml`:
        assert!(
            !html_clipboard.header().is_empty()
                || !txt_clipboard.header().is_empty()
                || !stdin.header().is_empty()
        );

        let context = context.set_state_ready_for_template();

        // Create the `Note` object.
        // You can plug in your own type (must impl. `Content`).
        let mut n =
            Note::<ContentString>::from_content_template(context, TemplateKind::FromClipboardYaml)
                .unwrap();
        let expected_body = "\nstdin body\nmy TXT clipboard\n\n";
        assert_eq!(n.content.body(), expected_body);
        // Check the title and subtitle in the note's header.
        assert_eq!(
            n.context
                .get(TMPL_VAR_FM_ALL)
                .unwrap()
                .get("fm_title")
                .unwrap()
                .as_str(),
            Some("my dir")
        );

        assert_eq!(
            n.context
                .get(TMPL_VAR_FM_ALL)
                .unwrap()
                .get("fm_subtitle")
                .unwrap()
                .as_str(),
            // Remember: in debug titles are very short. The code only works,
            // because the string is pure ASCII (not UTF-8).
            Some(&"this overwrites"[..TRUNC_LEN_MAX - 1])
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
            .contains(&"my dir--this overwrites"[..TRUNC_LEN_MAX - 1]));
        assert!(n.rendered_filename.is_file());
        let raw_note = fs::read_to_string(n.rendered_filename).unwrap();
        #[cfg(not(target_family = "windows"))]
        assert!(raw_note.starts_with("\u{feff}---\ntitle:        my dir"));
        #[cfg(target_family = "windows")]
        assert!(raw_note.starts_with("\u{feff}---\r\ntitle:        my dir"));
    }

    #[test]
    fn test_from_content_template4() {
        // Example with `TemplateKind::AnnotateFile`

        use crate::content::Content;
        use crate::content::ContentString;
        use crate::context::Context;
        use crate::note::Note;
        use crate::settings::Settings;
        use crate::settings::SETTINGS;
        use crate::template::TemplateKind;
        use parking_lot::RwLockWriteGuard;
        use std::env::temp_dir;
        use std::fs;

        // Prepare the test.
        let mut settings = SETTINGS.write();
        *settings = Settings::default();
        // This locks `SETTINGS` for further write access in this scope.
        let _settings = RwLockWriteGuard::<'_, _>::downgrade(settings);

        // Create some non-Tp-Note-file.
        let raw = "This simulates a non tp-note file";
        let non_notefile = temp_dir().join("20221030-some.pdf");
        fs::write(&non_notefile, raw.as_bytes()).unwrap();
        let expected = temp_dir().join("20221030-some.pdf--Note.md");
        let _ = fs::remove_file(&expected);

        // Run the test.
        // Store the path in `context`.
        let mut context = Context::from(&non_notefile).unwrap();
        let html_clipboard = ContentString::from_string(
            "my HTML clipboard\n".to_string(),
            "html_clipboard_header".to_string(),
            "html_clipboard".to_string(),
        );
        let txt_clipboard = ContentString::from_string(
            "my TXT clipboard\n".to_string(),
            "txt_clipboard_header".to_string(),
            "txt_clipboard".to_string(),
        );
        let stdin = ContentString::from_string_with_cr(
            "my stdin\n".to_string(),
            "stdin_header".to_string(),
            "stdin".to_string(),
        );
        let v = vec![&html_clipboard, &txt_clipboard, &stdin];

        context
            .insert_front_matter_and_raw_text_from_existing_content(&v)
            .unwrap();

        let context = context.set_state_ready_for_template();

        // Create the `Note` object.
        // You can plug in your own type (must impl. `Content`).
        let mut n =
            Note::<ContentString>::from_content_template(context, TemplateKind::AnnotateFile)
                .unwrap();
        let expected_body =
            "\n[20221030-some.pdf](<20221030-some.pdf>)\n\n---\n\nmy stdin\nmy TXT clipboard\n";
        assert_eq!(n.content.body(), expected_body);
        // Check the title and subtitle in the note's header.
        assert_eq!(
            n.context
                .get(TMPL_VAR_FM_ALL)
                .unwrap()
                .get("fm_title")
                .unwrap()
                .as_str(),
            Some("some.pdf")
        );
        assert_eq!(
            n.context
                .get(TMPL_VAR_FM_ALL)
                .unwrap()
                .get("fm_subtitle")
                .unwrap()
                .as_str(),
            Some("Note")
        );

        n.render_filename(TemplateKind::AnnotateFile).unwrap();
        n.set_next_unused_rendered_filename().unwrap();
        n.save().unwrap();

        // Check the new note file.
        assert_eq!(n.rendered_filename, expected);
        fs::remove_file(n.rendered_filename).unwrap();
    }
}
