//! Tp-Note's high level API.<!-- The low level API is documented
//! in the module `tpnote_lib::note`. -->
//!
//! How to integrate this in your text editor code?
//! First, call `create_new_note_or_synchronize_filename()`
//! with the first positional command line parameter `<path>`.
//! Then open the new text file with the returned path in your
//! text editor. After modifying the text, saving it and closing your
//! text editor, call `synchronize_filename()`.
//! The returned path points to the possibly renamed note file.
//!
//! Tp-Note is customizable at runtime by modifying its configuration stored in
//! `crate::config::LIB_CFG` before executing the functions in this
//! module (see type definition and documentation in `crate::config::LibCfg`).
//! All functions in this API are stateless.
//!
//!
//! ## Example with `TemplateKind::New`
//!
//! ```rust
//! use tpnote_lib::content::Content;
//! use tpnote_lib::content::ContentString;
//! use tpnote_lib::workflow::WorkflowBuilder;
//! use std::env::temp_dir;
//! use std::fs;
//! use std::path::Path;
//!
//! // Prepare test.
//! let notedir = temp_dir();
//!
//! let html_clipboard = ContentString::default();
//! let txt_clipboard = ContentString::default();
//! let stdin = ContentString::default();
//! // This is the condition to choose: `TemplateKind::New`:
//! assert!(html_clipboard.is_empty() && txt_clipboard.is_empty() &&stdin.is_empty());
//! // There are no inhibitor rules to change the `TemplateKind`.
//! let template_kind_filter = |tk|tk;
//!
//! // Build and run workflow.
//! let n = WorkflowBuilder::new(&notedir)
//!       // You can plug in your own type (must impl. `Content`).
//!      .upgrade::<ContentString, _>(
//!          "default", &html_clipboard, &txt_clipboard, &stdin, template_kind_filter)
//!      .build()
//!      .run()
//!      .unwrap();
//!
//! // Check result.
//! assert!(n.as_os_str().to_str().unwrap()
//!    .contains("--Note"));
//! assert!(n.is_file());
//! let raw_note = fs::read_to_string(n).unwrap();
//! #[cfg(not(target_family = "windows"))]
//! assert!(raw_note.starts_with("\u{feff}---\ntitle:"));
//! #[cfg(target_family = "windows")]
//! assert!(raw_note.starts_with("\u{feff}---\r\ntitle:"));
//! ```
//!
//! The internal data storage for the note's content is `ContentString`
//! which implements the `Content` trait. Now we modify slightly
//! the above example to showcase, how to overwrite
//! one of the trait's methods.
//!
//! ```rust
//! use std::path::Path;
//! use tpnote_lib::content::Content;
//! use tpnote_lib::content::ContentString;
//! use tpnote_lib::workflow::WorkflowBuilder;
//! use std::env::temp_dir;
//! use std::path::PathBuf;
//! use std::fs;
//! use std::fs::OpenOptions;
//! use std::io::Write;
//! use std::ops::Deref;
//!
//! #[derive(Default, Debug, Eq, PartialEq)]
//! // We need a newtype because of the orphan rule.
//! pub struct MyContentString(ContentString);
//!
//! impl From<String> for MyContentString {
//!     fn from(input: String) -> Self {
//!         MyContentString(ContentString::from(input))
//!     }
//! }
//!
//! impl AsRef<str> for MyContentString {
//!     fn as_ref(&self) -> &str {
//!         self.0.as_ref()
//!     }
//! }
//!
//! impl Content for MyContentString {
//!     // Now we overwrite one method to show how to plugin custom code.
//!     fn save_as(&self, new_file_path: &Path) -> Result<(), std::io::Error> {
//!         let mut outfile = OpenOptions::new()
//!             .write(true)
//!             .create(true)
//!             .open(&new_file_path)?;
//!         // We do not save the content to disk, we write intstead:
//!         write!(outfile, "Simulation")?;
//!         Ok(())
//!     }
//!    fn header(&self) -> &str {
//!        self.0.header()
//!    }
//!
//!    fn body(&self) -> &str {
//!        self.0.header()
//!    }
//!
//! }
//!
//! // Prepare test.
//! let notedir = temp_dir();
//!
//! let html_clipboard = MyContentString::default();
//! let txt_clipboard = MyContentString::default();
//! let stdin = MyContentString::default();
//! // This is the condition to choose: `TemplateKind::New`:
//! assert!(
//!     html_clipboard.is_empty() || txt_clipboard.is_empty() || stdin.is_empty());
//! // There are no inhibitor rules to change the `TemplateKind`.
//! let template_kind_filter = |tk|tk;
//!
//! // Build and run workflow.
//! let n = WorkflowBuilder::new(&notedir)
//!       // You can plug in your own type (must impl. `Content`).
//!      .upgrade::<MyContentString, _>(
//!          "default", &html_clipboard, &txt_clipboard, &stdin, template_kind_filter)
//!      .build()
//!      .run()
//!      .unwrap();
//!
//! // Check result.
//! assert!(n.as_os_str().to_str().unwrap()
//!    .contains("--Note"));
//! assert!(n.is_file());
//! let raw_note = fs::read_to_string(n).unwrap();
//! assert_eq!(raw_note, "Simulation");
//! ```

use crate::config::LocalLinkKind;
use crate::config::LIB_CFG;
#[cfg(feature = "viewer")]
use crate::config::TMPL_HTML_VAR_DOC_ERROR;
#[cfg(feature = "viewer")]
use crate::config::TMPL_HTML_VAR_DOC_TEXT;
use crate::content::Content;
use crate::context::Context;
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
    /// let raw = String::from(r#"---
    /// title: "My day"
    /// subtitle: "Note"
    /// ---
    /// Body text
    /// "#);
    ///
    /// // Start test
    /// let mut context = Context::from(Path::new("/path/to/note.md"));
    /// // We do not inject any JavaScript.
    /// context.insert(TMPL_HTML_VAR_VIEWER_DOC_JS, &"".to_string());
    /// // Render.
    /// let html = HtmlRenderer::viewer_page::<ContentString>(context, raw.into())
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
    /// title: "My day2"
    /// subtitle: "Note"
    /// ---
    /// Body text
    /// "#;
    /// let notefile = temp_dir().join("20221030-My day2--Note.md");
    /// fs::write(&notefile, raw.as_bytes()).unwrap();
    ///
    /// // Start test
    /// let mut context = Context::from(&notefile);
    /// // We do not inject any JavaScript.
    /// context.insert(TMPL_HTML_VAR_VIEWER_DOC_JS, &"".to_string());
    /// // Render.
    /// let content = ContentString::open(&context.path).unwrap();
    /// // You can plug in your own type (must impl. `Content`).
    /// let html = HtmlRenderer::viewer_page(context, content).unwrap();
    /// // Check the HTML rendition.
    /// assert!(html.starts_with("<!DOCTYPE html>\n<html"))
    /// ```
    pub fn viewer_page<T: Content>(context: Context, content: T) -> Result<String, NoteError> {
        let tmpl_html = &LIB_CFG.read_recursive().tmpl_html.viewer;
        Self::render(context, content, tmpl_html)
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
    /// let raw = String::from(r#"---
    /// title: "My day"
    /// subtitle: "Note"
    /// ---
    /// Body text
    /// "#);
    ///
    /// // Start test
    /// let mut context = Context::from(Path::new("/path/to/note.md"));
    /// // The exporter template does not insert any JavaScript.
    /// // Render.
    /// let html = HtmlRenderer::exporter_page::<ContentString>(context, raw.into())
    ///            .unwrap();
    /// // Check the HTML rendition.
    /// assert!(html.starts_with("<!DOCTYPE html>\n<html"))
    /// ```
    pub fn exporter_page<T: Content>(context: Context, content: T) -> Result<String, NoteError> {
        let tmpl_html = &LIB_CFG.read_recursive().tmpl_html.exporter;
        Self::render(context, content, tmpl_html)
    }

    /// Helper function.
    fn render<T: Content>(
        context: Context,
        content: T,
        tmpl_html: &str,
    ) -> Result<String, NoteError> {
        let note = Note::from_raw_text(context, content, TemplateKind::None)?;

        note.render_content_to_html(tmpl_html)
    }

    /// When the header can not be deserialized, the file located in
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
    /// let mut context = Context::from(&notefile);
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
        mut context: Context,
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
    pub fn save_exporter_page<T: Content>(
        doc_path: &Path,
        content: T,
        export_dir: &Path,
        local_link_kind: LocalLinkKind,
    ) -> Result<(), NoteError> {
        let context = Context::from(doc_path);

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
