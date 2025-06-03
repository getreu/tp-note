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
//! let v = vec![&html_clipboard, &txt_clipboard, &stdin];
//! // This is the condition to choose: `TemplateKind::New`:
//! assert!(html_clipboard.is_empty() && txt_clipboard.is_empty() &&stdin.is_empty());
//! // There are no inhibitor rules to change the `TemplateKind`.
//! let template_kind_filter = |tk|tk;
//!
//! // Build and run workflow.
//! let n = WorkflowBuilder::new(&notedir)
//!       // You can plug in your own type (must impl. `Content`).
//!      .upgrade::<ContentString, _>(
//!          "default", v, template_kind_filter)
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
//!    }
//!    // The rest we delegate.
//!    fn from_string(input: String, name: String) -> Self {
//!       MyContentString(
//!           ContentString::from_string(input, name))
//!    }
//!    fn header(&self) -> &str {
//!        self.0.header()
//!    }
//!    fn body(&self) -> &str {
//!        self.0.header()
//!    }
//!    fn name(&self) -> &str {
//!        self.0.name()
//!    }
//! }
//!
//! // Prepare test.
//! let notedir = temp_dir();
//!
//! let html_clipboard = MyContentString::default();
//! let txt_clipboard = MyContentString::default();
//! let stdin = MyContentString::default();
//! let v = vec![&html_clipboard, &txt_clipboard, &stdin];
//! // There are no inhibitor rules to change the `TemplateKind`.
//! let template_kind_filter = |tk|tk;
//!
//! // Build and run workflow.
//! let n = WorkflowBuilder::new(&notedir)
//!       // You can plug in your own type (must impl. `Content`).
//!      .upgrade::<MyContentString, _>(
//!          "default", v, template_kind_filter)
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
use crate::config::TMPL_VAR_FM_;
use crate::config::TMPL_VAR_FM_ALL;
use crate::config::TMPL_VAR_FM_FILENAME_SYNC;
use crate::config::TMPL_VAR_FM_NO_FILENAME_SYNC;
use crate::config::TMPL_VAR_FM_SCHEME;
use crate::content::Content;
use crate::context::Context;
use crate::error::NoteError;
use crate::html_renderer::HtmlRenderer;
use crate::note::Note;
use crate::settings::SETTINGS;
use crate::settings::SchemeSource;
use crate::settings::Settings;
use crate::template::TemplateKind;
use parking_lot::RwLockUpgradableReadGuard;
use std::path::Path;
use std::path::PathBuf;
use tera::Value;

/// Typestate of the `WorkflowBuilder`.
#[derive(Debug, Clone)]
pub struct WorkflowBuilder<W> {
    input: W,
}

/// In this state the workflow will only synchronize the filename.
#[derive(Debug, Clone)]
pub struct SyncFilename<'a> {
    path: &'a Path,
}

/// In this state the workflow will either synchronize the filename of an
/// existing note or, -if none exists- create a new note.
#[derive(Debug, Clone)]
pub struct SyncFilenameOrCreateNew<'a, T, F> {
    scheme_source: SchemeSource<'a>,
    path: &'a Path,
    clipboards: Vec<&'a T>,
    tk_filter: F,
    html_export: Option<(&'a Path, LocalLinkKind)>,
    force_lang: Option<&'a str>,
}

impl<'a> WorkflowBuilder<SyncFilename<'a>> {
    /// Constructor of all workflows. The `path` points
    /// 1. to an existing note file, or
    /// 2. to a directory where the new note should be created, or
    /// 3. to a non-Tp-Note file that will be annotated.
    ///
    /// For cases 2. and 3. upgrade the `WorkflowBuilder` with
    /// `upgrade()` to add additional input data.
    pub fn new(path: &'a Path) -> Self {
        Self {
            input: SyncFilename { path },
        }
    }

    /// Upgrade the `WorkflowBuilder` to enable also the creation of new note
    /// files. It requires providing additional input data:
    ///
    /// New notes are created by inserting `Tp-Note`'s environment
    /// in a template. The template set being used, is determined by
    /// `scheme_new_default`. If the note to be created exists already, append
    /// a so called `copy_counter` to the filename and try to save it again. In
    /// case this does not succeed either, increment the `copy_counter` until a
    /// free filename is found. The returned path points to the (new) note file
    /// on disk. Depending on the context, Tp-Note chooses one `TemplateKind`
    /// to operate (cf. `tpnote_lib::template::TemplateKind::from()`).
    /// The `tk-filter` allows to overwrite this choice, e.g. you may set
    /// `TemplateKind::None` under certain circumstances. This way the caller
    /// can disable the filename synchronization and inject behavior like
    /// `--no-filename-sync`.
    ///
    /// Some templates insert the content of the clipboard or the standard
    /// input pipe. The input data (can be empty) is provided with a
    /// vector of `Content` named `clipboards`. The templates expect text with
    /// markup or HTML. In case of HTML, the `Content.body` must start with
    /// `<!DOCTYPE html` or `<html`
    pub fn upgrade<T: Content, F: Fn(TemplateKind) -> TemplateKind>(
        self,
        scheme_new_default: &'a str,
        clipboards: Vec<&'a T>,
        tk_filter: F,
    ) -> WorkflowBuilder<SyncFilenameOrCreateNew<'a, T, F>> {
        WorkflowBuilder {
            input: SyncFilenameOrCreateNew {
                scheme_source: SchemeSource::SchemeNewDefault(scheme_new_default),
                path: self.input.path,
                clipboards,
                tk_filter,
                html_export: None,
                force_lang: None,
            },
        }
    }

    /// Finalize the build.
    pub fn build(self) -> Workflow<SyncFilename<'a>> {
        Workflow { input: self.input }
    }
}

impl<'a, T: Content, F: Fn(TemplateKind) -> TemplateKind>
    WorkflowBuilder<SyncFilenameOrCreateNew<'a, T, F>>
{
    /// Set a flag, that the workflow also stores an HTML-rendition of the
    /// note file next to it.
    /// This optional HTML rendition is performed just before returning and does
    /// not affect any above described operation.
    pub fn html_export(&mut self, path: &'a Path, local_link_kind: LocalLinkKind) {
        self.input.html_export = Some((path, local_link_kind));
    }

    /// Overwrite the default scheme.
    pub fn force_scheme(&mut self, scheme: &'a str) {
        self.input.scheme_source = SchemeSource::Force(scheme);
    }

    /// By default, the natural language, the note is written in is guessed
    /// from the title and subtitle. This disables the automatic guessing
    /// and forces the language.
    pub fn force_lang(&mut self, force_lang: &'a str) {
        self.input.force_lang = Some(force_lang);
    }

    /// Finalize the build.
    pub fn build(self) -> Workflow<SyncFilenameOrCreateNew<'a, T, F>> {
        Workflow { input: self.input }
    }
}

/// Holds the input data for the `run()` method.
#[derive(Debug, Clone)]
pub struct Workflow<W> {
    input: W,
}

impl Workflow<SyncFilename<'_>> {
    /// Starts the "synchronize filename" workflow. Errors can occur in
    /// various ways, see `NoteError`.
    ///
    /// First, the workflow opens the note file `path` on disk and read its
    /// YAML front matter. Then, it calculates from the front matter how the
    /// filename should be to be in sync. If it is different, rename the note on
    /// disk. Finally, it returns the note's new or existing filename. Repeated
    /// calls, will reload the environment variables, but not the configuration
    /// file. This function is stateless.
    ///
    /// Note: this method holds an (upgradeable read) lock on the `SETTINGS`
    /// object to ensure that the `SETTINGS` content does not change. The lock
    /// also prevents from concurrent execution.
    ///
    ///
    /// ## Example with `TemplateKind::SyncFilename`
    ///
    /// ```rust
    /// use tpnote_lib::content::ContentString;
    /// use tpnote_lib::workflow::WorkflowBuilder;
    /// use std::env::temp_dir;
    /// use std::fs;
    /// use std::path::Path;
    ///
    /// // Prepare test: create existing note.
    /// let raw = r#"
    ///
    /// ---
    /// title: "My day"
    /// subtitle: "Note"
    /// ---
    /// Body text
    /// "#;
    /// let notefile = temp_dir().join("20221030-hello.md");
    /// fs::write(&notefile, raw.as_bytes()).unwrap();
    ///
    /// let expected = temp_dir().join("20221030-My day--Note.md");
    /// let _ = fs::remove_file(&expected);
    ///
    /// // Build and run workflow.
    /// let n = WorkflowBuilder::new(&notefile)
    ///      .build()
    ///      // You can plug in your own type (must impl. `Content`).
    ///      .run::<ContentString>()
    ///      .unwrap();
    ///
    /// // Check result
    /// assert_eq!(n, expected);
    /// assert!(n.is_file());
    /// ```
    pub fn run<T: Content>(self) -> Result<PathBuf, NoteError> {
        // Prevent the rest to run in parallel, other threads will block when they
        // try to write.
        let mut settings = SETTINGS.upgradable_read();

        // Collect input data for templates.
        let context = Context::from(self.input.path)?;

        let content = <T>::open(self.input.path).unwrap_or_default();

        // This does not fill any templates,
        let mut n = Note::from_existing_content(context, content, TemplateKind::SyncFilename)?;

        synchronize_filename(&mut settings, &mut n)?;

        Ok(n.rendered_filename)
    }
}

impl<T: Content, F: Fn(TemplateKind) -> TemplateKind> Workflow<SyncFilenameOrCreateNew<'_, T, F>> {
    /// Starts the "synchronize filename or create a new note" workflow.
    /// Returns the note's new or existing filename. Repeated calls, will
    /// reload the environment variables, but not the configuration file. This
    /// function is stateless.
    /// Errors can occur in various ways, see `NoteError`.
    ///
    /// Note: this method holds an (upgradeable read) lock on the `SETTINGS`
    /// object to ensure that the `SETTINGS` content does not change. The lock
    /// also prevents from concurrent execution.
    ///
    ///
    /// ## Example with `TemplateKind::FromClipboard`
    ///
    /// ```rust
    /// use tpnote_lib::content::Content;
    /// use tpnote_lib::content::ContentString;
    /// use tpnote_lib::workflow::WorkflowBuilder;
    /// use std::env::temp_dir;
    /// use std::path::PathBuf;
    /// use std::fs;
    ///
    /// // Prepare test.
    /// let notedir = temp_dir();
    ///
    /// let html_clipboard = ContentString::from_string(
    ///     "my HTML clipboard\n".to_string(),
    ///     "html_clipboard".to_string()
    /// );
    /// let txt_clipboard = ContentString::from_string(
    ///     "my TXT clipboard\n".to_string(),
    ///     "txt_clipboard".to_string()
    /// );
    /// let stdin = ContentString::from_string(
    ///     "my stdin\n".to_string(),
    ///     "stdin".to_string()
    /// );
    /// let v = vec![&html_clipboard, &txt_clipboard, &stdin];
    /// // This is the condition to choose: `TemplateKind::FromClipboard`:
    /// assert!(html_clipboard.header().is_empty()
    ///            && txt_clipboard.header().is_empty()
    ///            && stdin.header().is_empty());
    /// assert!(!html_clipboard.body().is_empty() || !txt_clipboard.body().is_empty() || !stdin.body().is_empty());
    /// let template_kind_filter = |tk|tk;
    ///
    /// // Build and run workflow.
    /// let n = WorkflowBuilder::new(&notedir)
    ///       // You can plug in your own type (must impl. `Content`).
    ///      .upgrade::<ContentString, _>(
    ///            "default", v, template_kind_filter)
    ///      .build()
    ///      .run()
    ///      .unwrap();
    ///
    /// // Check result.
    /// assert!(n.as_os_str().to_str().unwrap()
    ///    .contains("my stdin--Note"));
    /// assert!(n.is_file());
    /// let raw_note = fs::read_to_string(n).unwrap();
    ///
    /// #[cfg(not(target_family = "windows"))]
    /// assert!(raw_note.starts_with(
    ///            "\u{feff}---\ntitle:        my stdin"));
    /// #[cfg(target_family = "windows")]
    /// assert!(raw_note.starts_with(
    ///            "\u{feff}---\r\ntitle:"));
    /// ```
    pub fn run(self) -> Result<PathBuf, NoteError> {
        // Prevent the rest to run in parallel, other threads will block when they
        // try to write.
        let mut settings = SETTINGS.upgradable_read();

        // Initialize settings.
        settings.with_upgraded(|settings| {
            settings.update(self.input.scheme_source, self.input.force_lang)
        })?;

        // First, generate a new note (if it does not exist), then parse its front_matter
        // and finally rename the file, if it is not in sync with its front matter.

        // Collect input data for templates.
        let context = Context::from(self.input.path)?;

        // `template_kind` will tell us what to do.
        let (template_kind, content) = TemplateKind::from(self.input.path);
        let template_kind = (self.input.tk_filter)(template_kind);

        let n = match template_kind {
            TemplateKind::FromDir | TemplateKind::AnnotateFile => {
                // CREATE A NEW NOTE WITH THE `TMPL_NEW_CONTENT` TEMPLATE
                // All these template do not refer to existing front matter,
                // as there is none yet.
                let context = context
                    .insert_front_matter_and_raw_text_from_existing_content(&self.input.clipboards)?
                    .set_state_ready_for_content_template();

                let mut n = Note::from_content_template(context, template_kind)?;
                n.render_filename(template_kind)?;
                // Check if the filename is not taken already
                n.set_next_unused_rendered_filename()?;
                n.save()?;
                n
            }

            TemplateKind::FromTextFile => {
                // This is part of the contract for this template:
                let content: T = content.unwrap();
                debug_assert!(&content.header().is_empty());
                debug_assert!(!&content.body().is_empty());

                let context = context
                    .insert_front_matter_and_raw_text_from_existing_content(&self.input.clipboards)?
                    .insert_front_matter_and_raw_text_from_existing_content(&vec![&content])?;

                let context = context.set_state_ready_for_content_template();

                let mut n = Note::from_content_template(context, TemplateKind::FromTextFile)?;
                // Render filename.
                n.render_filename(template_kind)?;

                // Save new note.
                let context_path = n.context.get_path().to_owned();
                n.set_next_unused_rendered_filename_or(&context_path)?;
                n.save_and_delete_from(&context_path)?;
                n
            }

            TemplateKind::SyncFilename => {
                let mut n = Note::from_existing_content(
                    context,
                    content.unwrap(),
                    TemplateKind::SyncFilename,
                )?;

                synchronize_filename(&mut settings, &mut n)?;
                n
            }

            TemplateKind::None => {
                Note::from_existing_content(context, content.unwrap(), template_kind)?
            }
        };

        // If no new filename was rendered, return the old one.
        let mut n = n;
        if n.rendered_filename == PathBuf::new() {
            n.rendered_filename = n.context.get_path().to_owned();
        }

        // Export HTML rendition, if wanted.
        if let Some((export_dir, local_link_kind)) = self.input.html_export {
            HtmlRenderer::save_exporter_page(
                &n.rendered_filename,
                n.content,
                export_dir,
                local_link_kind,
            )?;
        }

        Ok(n.rendered_filename)
    }
}

///
/// Helper function. We take `RwLockUpgradableReadGuard<Settings>` as parameter
/// with a unique `mut` pointer because:
/// 1. It serves as a lock to prevent several instances of
///    `synchronize_filename` from running in parallel.
/// 2. We need write access to `SETTINGS` in this function.
fn synchronize_filename<T: Content>(
    settings: &mut RwLockUpgradableReadGuard<Settings>,
    note: &mut Note<T>,
) -> Result<(), NoteError> {
    let no_filename_sync = match (
        note.context
            .get(TMPL_VAR_FM_ALL)
            .and_then(|v| v.get(TMPL_VAR_FM_FILENAME_SYNC)),
        note.context
            .get(TMPL_VAR_FM_ALL)
            .and_then(|v| v.get(TMPL_VAR_FM_NO_FILENAME_SYNC)),
    ) {
        // By default we sync.
        (None, None) => false,
        (None, Some(Value::Bool(nsync))) => *nsync,
        (None, Some(_)) => true,
        (Some(Value::Bool(sync)), None) => !*sync,
        _ => false,
    };

    if no_filename_sync {
        log::info!(
            "Filename synchronisation disabled with the front matter field: `{}: {}`",
            TMPL_VAR_FM_FILENAME_SYNC.trim_start_matches(TMPL_VAR_FM_),
            !no_filename_sync
        );
        return Ok(());
    }

    // Shall we switch the `settings.current_theme`?
    // If `fm_scheme` is defined, prefer this value.
    match note
        .context
        .get(TMPL_VAR_FM_ALL)
        .and_then(|v| v.get(TMPL_VAR_FM_SCHEME))
    {
        Some(Value::String(s)) if !s.is_empty() => {
            // Initialize `SETTINGS`.
            settings
                .with_upgraded(|settings| settings.update_current_scheme(SchemeSource::Force(s)))?;
            log::info!("Switch to scheme `{}` as indicated in front matter", s);
        }
        Some(Value::String(_)) | None => {
            // Initialize `SETTINGS`.
            settings.with_upgraded(|settings| {
                settings.update_current_scheme(SchemeSource::SchemeSyncDefault)
            })?;
        }
        Some(_) => {
            return Err(NoteError::FrontMatterFieldIsNotString {
                field_name: TMPL_VAR_FM_SCHEME.to_string(),
            });
        }
    };

    note.render_filename(TemplateKind::SyncFilename)?;

    let path = note.context.get_path().to_owned();
    note.set_next_unused_rendered_filename_or(&path)?;
    // Silently fails is source and target are identical.
    note.rename_file_from(note.context.get_path())?;

    Ok(())
}
