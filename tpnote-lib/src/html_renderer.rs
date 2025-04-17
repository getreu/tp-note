//! Tp-Note's high level HTML rendering API.
//!
//! A set of functions that take a `Context` type and a `Content` type (or raw
//! text) and return the HTML rendition of the content. The API is completely
//! stateless. All functions read the `LIB_CFG` global variable to read the
//! configuration stored in `LibCfg.tmpl_html`.

use crate::config::LocalLinkKind;
use crate::config::LIB_CFG;
#[cfg(feature = "viewer")]
use crate::config::TMPL_HTML_VAR_DOC_ERROR;
#[cfg(feature = "viewer")]
use crate::config::TMPL_HTML_VAR_DOC_TEXT;
use crate::content::Content;
use crate::context::Context;
use crate::context::HasSettings;
use crate::error::NoteError;
#[cfg(feature = "viewer")]
use crate::filter::TERA;
use crate::html::rewrite_links;
use crate::html::HTML_EXT;
use crate::note::Note;
#[cfg(feature = "viewer")]
use crate::note::ONE_OFF_TEMPLATE_NAME;
#[cfg(feature = "viewer")]
use crate::note_error_tera_template;
use crate::template::TemplateKind;
use parking_lot::RwLock;
use std::collections::HashSet;
use std::fs::OpenOptions;
use std::io;
use std::io::Write;
use std::path::Path;
use std::path::PathBuf;
use std::str::FromStr;
use std::sync::Arc;
#[cfg(feature = "viewer")]
use tera::Tera;

/// High level API to render a note providing its `content` and some `context`.
pub struct HtmlRenderer;

impl HtmlRenderer {
    /// Returns the HTML rendition of a `ContentString`. The markup
    /// rendition engine is determined, by the file extension of the variable
    /// `context.path`. The resulting HTML and other HTML template variables
    /// originating from `context` are inserted into the `TMPL_HTML_VIEWER`
    /// template (which can be replaced at runtime) before being returned. This
    /// function is stateless.
    ///
    /// ```rust
    /// use tpnote_lib::config::TMPL_HTML_VAR_VIEWER_DOC_JS;
    /// use tpnote_lib::content::Content;
    /// use tpnote_lib::content::ContentString;
    /// use tpnote_lib::context::Context;
    /// use tpnote_lib::html_renderer::HtmlRenderer;
    /// use std::env::temp_dir;
    /// use std::fs;
    /// use std::path::Path;
    ///
    /// // Prepare test: create existing note file.
    /// let content = ContentString::from_string(String::from(r#"---
    /// title: My day
    /// subtitle: Note
    /// ---
    /// Body text
    /// "#), "doc_header".to_string(), "doc".to_string());
    ///
    /// // Start test
    /// let mut context = Context::from(Path::new("/path/to/note.md")).unwrap();
    /// // We do not inject any JavaScript.
    /// context.insert(TMPL_HTML_VAR_VIEWER_DOC_JS, &"".to_string());
    /// // Render.
    /// let html = HtmlRenderer::viewer_page::<ContentString>(context, content)
    ///            .unwrap();
    /// // Check the HTML rendition.
    /// assert!(html.starts_with("<!DOCTYPE html>\n<html"))
    /// ```
    ///
    /// A more elaborated example that reads from disk:
    ///
    /// ```rust
    /// use tpnote_lib::config::LIB_CFG;
    /// use tpnote_lib::config::TMPL_HTML_VAR_VIEWER_DOC_JS;
    /// use tpnote_lib::content::Content;
    /// use tpnote_lib::content::ContentString;
    /// use tpnote_lib::context::Context;
    /// use tpnote_lib::html_renderer::HtmlRenderer;
    /// use std::env::temp_dir;
    /// use std::fs;
    ///
    /// // Prepare test: create existing note file.
    /// let raw = r#"---
    /// title: My day2
    /// subtitle: Note
    /// ---
    /// Body text
    /// "#;
    /// let notefile = temp_dir().join("20221030-My day2--Note.md");
    /// fs::write(&notefile, raw.as_bytes()).unwrap();
    ///
    /// // Start test
    /// let mut context = Context::from(&notefile).unwrap();
    /// // We do not inject any JavaScript.
    /// context.insert(TMPL_HTML_VAR_VIEWER_DOC_JS, &"".to_string());
    /// // Render.
    /// let content = ContentString::open(&context.path).unwrap();
    /// // You can plug in your own type (must impl. `Content`).
    /// let html = HtmlRenderer::viewer_page(context, content).unwrap();
    /// // Check the HTML rendition.
    /// assert!(html.starts_with("<!DOCTYPE html>\n<html"))
    /// ```
    pub fn viewer_page<T: Content>(
        context: Context<HasSettings>,
        content: T,
    ) -> Result<String, NoteError> {
        let tmpl_html = &LIB_CFG.read_recursive().tmpl_html.viewer;
        HtmlRenderer::render(context, content, tmpl_html)
    }

    /// Returns the HTML rendition of a `ContentString`. The markup rendition
    /// engine is determined, by the file extension of the variable `context.path`.
    /// The resulting HTML and other HTML template variables originating from
    /// `context` are inserted into the `TMPL_HTML_EXPORTER` template (which can be
    /// replaced at runtime) before being returned. This function is stateless.
    ///
    /// ```rust
    /// use tpnote_lib::config::TMPL_HTML_VAR_VIEWER_DOC_JS;
    /// use tpnote_lib::content::Content;
    /// use tpnote_lib::content::ContentString;
    /// use tpnote_lib::context::Context;
    /// use tpnote_lib::html_renderer::HtmlRenderer;
    /// use std::env::temp_dir;
    /// use std::fs;
    /// use std::path::Path;
    ///
    /// // Prepare test: create existing note file.
    /// let content= ContentString::from_string(String::from(r#"---
    /// title: "My day"
    /// subtitle: "Note"
    /// ---
    /// Body text
    /// "#), "doc_header".to_string(), "doc".to_string());
    ///
    /// // Start test
    /// let mut context = Context::from(Path::new("/path/to/note.md")).unwrap();
    /// // Render.
    /// let html = HtmlRenderer::exporter_page::<ContentString>(context, content)
    ///            .unwrap();
    /// // Check the HTML rendition.
    /// assert!(html.starts_with("<!DOCTYPE html>\n<html"))
    /// ```
    pub fn exporter_page<T: Content>(
        context: Context<HasSettings>,
        content: T,
    ) -> Result<String, NoteError> {
        let tmpl_html = &LIB_CFG.read_recursive().tmpl_html.exporter;
        HtmlRenderer::render(context, content, tmpl_html)
    }

    /// Helper function.
    fn render<T: Content>(
        context: Context<HasSettings>,
        content: T,
        tmpl_html: &str,
    ) -> Result<String, NoteError> {
        let note = Note::from_raw_text(context, content, TemplateKind::None)?;

        note.render_content_to_html(tmpl_html)
    }

    /// When the header cannot be deserialized, the file located in
    /// `context.path` is rendered as "Error HTML page".
    /// The erroneous content is rendered to html with
    /// `parse_hyperlinks::renderer::text_rawlinks2html` and inserted in
    /// the `TMPL_HTML_VIEWER_ERROR` template (can be replace at runtime).
    /// This template expects the template variables `TMPL_VAR_PATH`
    /// and `TMPL_HTML_VAR_VIEWER_DOC_JS` in `context` to be set.
    /// NB: The value of `TMPL_VAR_PATH` equals `context.path`.
    ///
    /// ```rust
    /// use tpnote_lib::config::LIB_CFG;
    /// use tpnote_lib::config::TMPL_HTML_VAR_DOC_ERROR;
    /// use tpnote_lib::config::TMPL_HTML_VAR_VIEWER_DOC_JS;
    /// use tpnote_lib::content::Content;
    /// use tpnote_lib::content::ContentString;
    /// use tpnote_lib::context::Context;
    /// use tpnote_lib::error::NoteError;
    /// use tpnote_lib::html_renderer::HtmlRenderer;
    /// use std::env::temp_dir;
    /// use std::fs;
    ///
    /// // Prepare test: create existing erroneous note file.
    /// let raw_error = r#"---
    /// title: "My day3"
    /// subtitle: "Note"
    /// --
    /// Body text
    /// "#;
    /// let notefile = temp_dir().join("20221030-My day3--Note.md");
    /// fs::write(&notefile, raw_error.as_bytes()).unwrap();
    /// let mut context = Context::from(&notefile);
    /// let e = NoteError::FrontMatterFieldMissing { field_name: "title".to_string() };
    ///
    /// // Start test
    /// let mut context = Context::from(&notefile).unwrap();
    /// // We do not inject any JavaScript.
    /// context.insert(TMPL_HTML_VAR_VIEWER_DOC_JS, "");
    /// // Render.
    /// // Read from file.
    /// // You can plug in your own type (must impl. `Content`).
    /// let content = ContentString::open(&context.path).unwrap();
    /// let html = HtmlRenderer::error_page(
    ///               context, content, &e.to_string()).unwrap();
    /// // Check the HTML rendition.
    /// assert!(html.starts_with("<!DOCTYPE html>\n<html"))
    /// ```
    #[cfg(feature = "viewer")]
    pub fn error_page<T: Content>(
        mut context: Context<HasSettings>,
        note_erroneous_content: T,
        error_message: &str,
    ) -> Result<String, NoteError> {
        // Insert.
        context.insert(TMPL_HTML_VAR_DOC_ERROR, error_message);
        context.insert(TMPL_HTML_VAR_DOC_TEXT, &note_erroneous_content.as_str());

        let tmpl_html = &LIB_CFG.read_recursive().tmpl_html.viewer_error;

        // Apply template.
        let mut tera = Tera::default();
        // Switch `autoescape_on()` only for HTML templates.
        tera.autoescape_on(vec![ONE_OFF_TEMPLATE_NAME]);
        tera.extend(&TERA)?;
        let html = tera
            .render_str(tmpl_html, &context)
            .map_err(|e| note_error_tera_template!(e, "[html_tmpl] viewer_error".to_string()))?;
        Ok(html)
    }

    /// Renders `doc_path` with `content` into HTML and saves the result in
    /// `export_dir`. If `export_dir` is the empty string, the directory of
    /// `doc_path` is used. `-` dumps the rendition to STDOUT. The filename
    /// of the html rendition is the same as in `doc_path`, but with `.html`
    /// appended.
    ///
    /// ```rust
    /// use tpnote_lib::config::LIB_CFG;
    /// use tpnote_lib::config::TMPL_HTML_VAR_VIEWER_DOC_JS;
    /// use tpnote_lib::config::LocalLinkKind;
    /// use tpnote_lib::content::Content;
    /// use tpnote_lib::content::ContentString;
    /// use tpnote_lib::context::Context;
    /// use tpnote_lib::html_renderer::HtmlRenderer;
    /// use std::env::temp_dir;
    /// use std::fs;
    /// use std::path::Path;
    ///
    /// // Prepare test: create existing note file.
    /// let raw = r#"---
    /// title: "My day3"
    /// subtitle: "Note"
    /// ---
    /// Body text
    /// "#;
    /// let notefile = temp_dir().join("20221030-My day3--Note.md");
    /// fs::write(&notefile, raw.as_bytes()).unwrap();
    ///
    /// // Start test
    /// let content = ContentString::open(&notefile).unwrap();
    /// // You can plug in your own type (must impl. `Content`).
    /// HtmlRenderer::save_exporter_page(
    ///        &notefile, content, Path::new(""), LocalLinkKind::Long).unwrap();
    /// // Check the HTML rendition.
    /// let expected_file = temp_dir().join("20221030-My day3--Note.md.html");
    /// let html = fs::read_to_string(expected_file).unwrap();
    /// assert!(html.starts_with("<!DOCTYPE html>\n<html"))
    /// ```
    pub fn save_exporter_page<T: Content>(
        doc_path: &Path,
        content: T,
        export_dir: &Path,
        local_link_kind: LocalLinkKind,
    ) -> Result<(), NoteError> {
        let context = Context::from(doc_path)?;

        let doc_path = context.path.clone();
        let doc_dir = context.dir_path.clone();

        // Determine filename of html-file.
        let html_path = match export_dir {
            p if p == Path::new("") => {
                let mut s = doc_path.as_path().to_str().unwrap_or_default().to_string();
                s.push_str(HTML_EXT);
                PathBuf::from_str(&s).unwrap_or_default()
            }
            p if p == Path::new("-") => PathBuf::new(),
            p => {
                let mut html_filename = doc_path
                    .file_name()
                    .unwrap_or_default()
                    .to_str()
                    .unwrap_or_default()
                    .to_string();
                html_filename.push_str(HTML_EXT);
                let mut p = p.to_path_buf();
                p.push(PathBuf::from(html_filename));
                p
            }
        };

        if html_path == Path::new("") {
            log::debug!("Rendering HTML to STDOUT (`{:?}`)", export_dir);
        } else {
            log::debug!("Rendering HTML into: {:?}", html_path);
        };

        // These must live longer than `writeable`, and thus are declared first:
        let (mut stdout_write, mut file_write);
        // We need to ascribe the type to get dynamic dispatch.
        let writeable: &mut dyn Write = if html_path == Path::new("") {
            stdout_write = io::stdout();
            &mut stdout_write
        } else {
            file_write = OpenOptions::new()
                .write(true)
                .create(true)
                .truncate(true)
                .open(&html_path)?;
            &mut file_write
        };

        // Render HTML.
        let root_path = context.root_path.clone();
        let html = Self::exporter_page(context, content)?;
        let html = rewrite_links(
            html,
            &root_path,
            &doc_dir,
            local_link_kind,
            // Do append `.html` to `.md` in links.
            true,
            Arc::new(RwLock::new(HashSet::new())),
        );

        // Write HTML rendition.
        writeable.write_all(html.as_bytes())?;
        Ok(())
    }
}
