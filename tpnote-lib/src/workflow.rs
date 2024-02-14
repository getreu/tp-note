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
//! use tpnote_lib::workflow::synchronize_filename;
//! use tpnote_lib::workflow::create_new_note_or_synchronize_filename;
//! use std::env::temp_dir;
//! use std::fs;
//! use std::path::Path;
//!
//! // Prepare test.
//! let notedir = temp_dir();
//!
//! let clipboard = ContentString::default();
//! let stdin = ContentString::default();
//! // This is the condition to choose: `TemplateKind::New`:
//! assert!(clipboard.is_empty() || stdin.is_empty());
//! // There are no inhibitor rules to change the `TemplateKind`.
//! let template_kind_filter = |tk|tk;
//!
//! // Start test.
//! // You can plug in your own type (must impl. `Content`).
//! let n = create_new_note_or_synchronize_filename::<ContentString, _>(
//!        &notedir, &clipboard, &stdin, template_kind_filter,
//!        &None, "default", None, None).unwrap();
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
//! use tpnote_lib::workflow::create_new_note_or_synchronize_filename;
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
//! let clipboard = MyContentString::default();
//! let stdin = MyContentString::default();
//! // This is the condition to choose: `TemplateKind::New`:
//! assert!(clipboard.is_empty() || stdin.is_empty());
//! // There are no inhibitor rules to change the `TemplateKind`.
//! let template_kind_filter = |tk|tk;
//!
//! // Start test.
//! // Here we plugin our own type (must implement `Content`).
//! let n = create_new_note_or_synchronize_filename::<MyContentString, _>(
//!        &notedir, &clipboard, &stdin, template_kind_filter,
//!        &None, "default", None, None).unwrap();
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
use crate::config::TMPL_HTML_VAR_DOC_TEXT;
use crate::config::TMPL_VAR_CLIPBOARD;
use crate::config::TMPL_VAR_CLIPBOARD_HEADER;
use crate::config::TMPL_VAR_FM_;
use crate::config::TMPL_VAR_FM_FILENAME_SYNC;
use crate::config::TMPL_VAR_FM_NO_FILENAME_SYNC;
use crate::config::TMPL_VAR_FM_SCHEME;
use crate::config::TMPL_VAR_STDIN;
use crate::config::TMPL_VAR_STDIN_HEADER;
use crate::content::Content;
use crate::context::Context;
use crate::error::NoteError;
#[cfg(feature = "viewer")]
use crate::filter::TERA;
use crate::note::Note;
#[cfg(feature = "viewer")]
use crate::note::ONE_OFF_TEMPLATE_NAME;
#[cfg(feature = "viewer")]
use crate::note_error_tera_template;
use crate::settings::SchemeSource;
use crate::settings::Settings;
use crate::settings::SETTINGS;
use crate::template::TemplateKind;
use parking_lot::RwLockUpgradableReadGuard;
use std::path::Path;
use std::path::PathBuf;
#[cfg(feature = "viewer")]
use tera::Tera;
use tera::Value;

/// Typestate of the `WorkflowBuilder`.
pub struct WorkflowBuilder<W> {
    input: W,
}

/// In this state the workflow will only synchronize the filename.
pub struct SyncFilename<'a> {
    path: &'a Path,
}

/// In this state the workflow will either synchronize the filename of an
/// existing note or, -if none exists- create a new note.
pub struct SyncFilenameOrCreateNew<'a, T, F> {
    scheme_new_default: &'a str,
    path: &'a Path,
    clipboard: &'a T,
    stdin: &'a T,
    tk_filter: F,
    html_export: &'a Option<(PathBuf, LocalLinkKind)>,
    force_scheme: Option<&'a str>,
    force_lang: Option<&'a str>,
}

impl<'a> WorkflowBuilder<SyncFilename<'a>> {
    /// Constructor of all workflows. The `path` points
    /// 1. to an existing note file, or
    /// 2. to a directory where the new note should be created, or
    /// 3. to a non-Tp-Note file that will be annotated.
    ///
    /// For cases 2. and 3. upgrade the `WorkflowBuilder` with
    /// `sync_or_create_new` to add additional input data.
    pub fn new(path: &'a Path) -> Self {
        Self {
            input: SyncFilename { path },
        }
    }

    /// Upgrade the `WorkflowBuilder` to enable also the creation of new note
    /// files. It requires providing additinal input data:
    ///
    /// New notes are created by inserting `Tp-Note`'s environment
    /// in a template. The template set being used, is detemined by
    /// `scheme_new_default`. If the note to be created exists already, append
    /// a so called `copy_counter` to the filename and try to save it again. In
    /// case this does not succeed either, increment the `copy_counter` until a
    /// free filename is found. The returned path points to the (new) note file
    /// on disk. Depending on the context, Tp-Note chooses one `TemplateKind`
    /// to operate (c.f. `tpnote_lib::template::TemplateKind::from()`).
    /// The `tk-filter` allows to overwrite this choice, e.g. you may set
    /// `TemplateKind::None` under certain circumstances. This way the caller
    /// can disable the filename synchronization and inject behavior like
    /// `--no-filename-sync`.
    ///
    /// Some templates insert the content of the clipboard or the standard
    /// input pipe. The input data (can be empty) must be provided with the
    /// parameters `clipboard` and `stdin`.
    pub fn upgrade<T: Content, F: Fn(TemplateKind) -> TemplateKind>(
        self,
        scheme_new_default: &'a str,
        clipboard: &'a T,
        stdin: &'a T,
        tk_filter: F,
    ) -> WorkflowBuilder<SyncFilenameOrCreateNew<'a, T, F>> {
        WorkflowBuilder {
            input: SyncFilenameOrCreateNew {
                path: self.input.path,
                scheme_new_default,
                clipboard,
                stdin,
                tk_filter,
                html_export: &None,
                force_scheme: None,
                force_lang: None,
            },
        }
    }

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
    ///      // You can plug in your own type (must impl. `Content`).
    ///      .run::<ContentString>()
    ///      .unwrap();
    ///
    /// // Check result
    /// assert_eq!(n, expected);
    /// assert!(n.is_file());
    /// ```
    pub fn run<T: Content>(self) -> Result<PathBuf, NoteError> {
        synchronize_filename::<T>(self.input.path)
    }
}

impl<'a, T: Content, F: Fn(TemplateKind) -> TemplateKind>
    WorkflowBuilder<SyncFilenameOrCreateNew<'a, T, F>>
{
    /// Set a flag, that the workflow also stores an HTML-rendition of the
    /// note file next to it.
    /// This optional HTML rendition is performed just before returning and does
    /// not affect any above described operation.
    pub fn html_export(&mut self, html_export: &'a Option<(PathBuf, LocalLinkKind)>) {
        self.input.html_export = html_export;
    }

    /// Overwrite the default scheme.
    pub fn force_scheme(&mut self, force_scheme: &'a str) {
        self.input.force_scheme = Some(force_scheme);
    }

    /// By default, the natural language, the note is written in is guessed
    /// from the title and subtitle. This disables the automatic guessing
    /// and forces the language.
    pub fn force_lang(&mut self, force_lang: &'a str) {
        self.input.force_lang = Some(force_lang);
    }

    /// Starts the "synchronize filename or create a new note" workflow.
    /// Returns the note's new or existing filename.  Repeated calls, will
    /// reload the environment variables, but not  the configuration file. This
    /// function is stateless.
    /// Errors can occur in various ways, see `NoteError`.
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
    /// let clipboard = ContentString::from("my clipboard\n".to_string());
    /// let stdin = ContentString::from("my stdin\n".to_string());
    /// // This is the condition to choose: `TemplateKind::FromClipboard`:
    /// assert!(clipboard.header().is_empty() && stdin.header().is_empty());
    /// assert!(!clipboard.body().is_empty() || !stdin.body().is_empty());
    /// let template_kind_filter = |tk|tk;
    ///
    /// // Build and run workflow.
    /// let n = WorkflowBuilder::new(&notedir)
    ///       // You can plug in your own type (must impl. `Content`).
    ///      .upgrade::<ContentString, _>(
    ///            "default", &clipboard, &stdin, template_kind_filter)
    ///      .run()
    ///      .unwrap();
    ///
    /// // Check result.
    /// assert!(n.as_os_str().to_str().unwrap()
    ///    .contains("my stdin-my clipboard--Note"));
    /// assert!(n.is_file());
    /// let raw_note = fs::read_to_string(n).unwrap();
    ///
    /// #[cfg(not(target_family = "windows"))]
    /// assert!(raw_note.starts_with(
    ///            "\u{feff}---\ntitle:        |\n  my stdin\n  my clipboard"));
    /// #[cfg(target_family = "windows")]
    /// assert!(raw_note.starts_with(
    ///            "\u{feff}---\r\ntitle:        |"));
    /// ```
    pub fn run(self) -> Result<PathBuf, NoteError> {
        create_new_note_or_synchronize_filename::<T, F>(
            self.input.path,
            self.input.clipboard,
            self.input.stdin,
            self.input.tk_filter,
            self.input.html_export,
            self.input.scheme_new_default,
            self.input.force_scheme,
            self.input.force_lang,
        )
    }
}

/// Open the note file `path` on disk and read its YAML front matter.
/// Then calculate from the front matter how the filename should be to
/// be in sync. If it is different, rename the note on disk.
/// Returns the note's new or existing filename.
/// Repeated calls, will reload the environment variables, but not
/// the configuration file. This function is stateless.
///
///
/// ## Example with `TemplateKind::SyncFilename`
///
/// ```rust
/// use tpnote_lib::content::ContentString;
/// use tpnote_lib::workflow::synchronize_filename;
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
/// // Start test.
/// // You can plug in your own type (must impl. `Content`).
/// let n = synchronize_filename::<ContentString>(&notefile).unwrap();
///
/// // Check result
/// assert_eq!(n, expected);
/// assert!(n.is_file());
/// ```
pub fn synchronize_filename<T: Content>(path: &Path) -> Result<PathBuf, NoteError> {
    // Prevent the rest to run in parallel, other threads will block when they
    // try to write.
    let mut settings = SETTINGS.upgradable_read();

    // Collect input data for templates.
    let context = Context::from(path);

    let content = <T>::open(path).unwrap_or_default();

    // This does not fill any templates,
    let mut n = Note::from_raw_text(context, content, TemplateKind::SyncFilename)?;

    synchronize::<T>(&mut settings, &mut n)?;

    Ok(n.rendered_filename)
}

#[inline]
/// Create a new note by inserting `Tp-Note`'s environment in a template. If
/// the note to be created exists already, append a so called `copy_counter` to
/// the filename and try to save it again. In case this does not succeed either,
/// increment the `copy_counter` until a free filename is found.
/// The returned path points to the (new) note file on disk.
/// Depending on the context, Tp-Note chooses one `TemplateKind` to operate
/// (c.f. `tpnote_lib::template::TemplateKind::from()`).
/// The `tk-filter` allows to overwrite this choice, e.g. you may set
/// `TemplateKind::None` under certain circumstances. This way the caller
/// can inject command line parameters like `--no-filename-sync`.
/// If `html_export = Some((dir, local_link_kind))`, the function acts like
/// like described above, but in addition it renders
/// the note's content into HTML and saves the `.html` file in the
/// directory `dir`. This optional HTML rendition is performed just before
/// returning and does not affect any above described operation.
/// `force_lang` disables the automatic language detection and uses `force_lang`
/// instead; or, if `-` use the environment variable `TPNOTE_LANG` or, - if not
/// defined - use the user's default language as reported from the operating
/// system.
///
/// Returns the note's new or existing filename.
/// Repeated calls, will reload the environment variables, but not
/// the configuration file. This function is stateless.
///
///
/// ## Example with `TemplateKind::FromClipboard`
///
/// ```rust
/// use tpnote_lib::content::Content;
/// use tpnote_lib::content::ContentString;
/// use tpnote_lib::workflow::create_new_note_or_synchronize_filename;
/// use std::env::temp_dir;
/// use std::path::PathBuf;
/// use std::fs;
///
/// // Prepare test.
/// let notedir = temp_dir();
///
/// let clipboard = ContentString::from("my clipboard\n".to_string());
/// let stdin = ContentString::from("my stdin\n".to_string());
/// // This is the condition to choose: `TemplateKind::FromClipboard`:
/// assert!(clipboard.header().is_empty() && stdin.header().is_empty());
/// assert!(!clipboard.body().is_empty() || !stdin.body().is_empty());
/// let template_kind_filter = |tk|tk;
///
/// // Start test.
/// // You can plug in your own type (must impl. `Content`).
/// let n = create_new_note_or_synchronize_filename::<ContentString, _>(
///        &notedir, &clipboard, &stdin, template_kind_filter,
///        &None, "default", None, None).unwrap();
/// // Check result.
/// assert!(n.as_os_str().to_str().unwrap()
///    .contains("my stdin-my clipboard--Note"));
/// assert!(n.is_file());
/// let raw_note = fs::read_to_string(n).unwrap();
///
/// #[cfg(not(target_family = "windows"))]
/// assert!(raw_note.starts_with(
///            "\u{feff}---\ntitle:        |\n  my stdin\n  my clipboard"));
/// #[cfg(target_family = "windows")]
/// assert!(raw_note.starts_with(
///            "\u{feff}---\r\ntitle:        |"));
/// ```
#[allow(clippy::too_many_arguments)]
pub fn create_new_note_or_synchronize_filename<T, F>(
    path: &Path,
    clipboard: &T,
    stdin: &T,
    tk_filter: F,
    html_export: &Option<(PathBuf, LocalLinkKind)>,
    scheme_new_default: &str,
    force_scheme: Option<&str>,
    force_lang: Option<&str>,
) -> Result<PathBuf, NoteError>
where
    T: Content,
    F: Fn(TemplateKind) -> TemplateKind,
{
    let scheme_sorce = match force_scheme {
        Some(s) => SchemeSource::Force(s),
        None => SchemeSource::SchemeNewDefault(scheme_new_default),
    };

    // Prevent the rest to run in parallel, other threads will block when they
    // try to write.
    let mut settings = SETTINGS.upgradable_read();

    // Initialize settings.
    settings.with_upgraded(|settings| settings.update(scheme_sorce, force_lang))?;

    // First, generate a new note (if it does not exist), then parse its front_matter
    // and finally rename the file, if it is not in sync with its front matter.

    // Collect input data for templates.
    let mut context = Context::from(path);
    context.insert_content(TMPL_VAR_CLIPBOARD, TMPL_VAR_CLIPBOARD_HEADER, clipboard)?;
    context.insert_content(TMPL_VAR_STDIN, TMPL_VAR_STDIN_HEADER, stdin)?;

    // `template_king` will tell us what to do.
    let (template_kind, content) = TemplateKind::from::<T>(path, clipboard, stdin);
    let template_kind = tk_filter(template_kind);

    let n = match template_kind {
        TemplateKind::FromDir
        | TemplateKind::FromClipboardYaml
        | TemplateKind::FromClipboard
        | TemplateKind::AnnotateFile => {
            // CREATE A NEW NOTE WITH `TMPL_NEW_CONTENT` TEMPLATE
            let mut n = Note::from_content_template(context, template_kind)?;
            n.render_filename(template_kind)?;
            // Check if the filename is not taken already
            n.set_next_unused_rendered_filename()?;
            n.save()?;
            n
        }

        TemplateKind::FromTextFile => {
            let mut n = Note::from_raw_text(context, content.unwrap(), template_kind)?;
            // Render filename.
            n.render_filename(template_kind)?;

            // Save new note.
            let context_path = n.context.path.clone();
            n.set_next_unused_rendered_filename_or(&context_path)?;
            n.save_and_delete_from(&context_path)?;
            n
        }

        TemplateKind::SyncFilename => {
            let mut n = Note::from_raw_text(context, content.unwrap(), TemplateKind::SyncFilename)?;

            synchronize::<T>(&mut settings, &mut n)?;
            n
        }

        TemplateKind::None => Note::from_raw_text(context, content.unwrap(), template_kind)?,
    };

    // Export HTML rendition, if wanted.
    if let Some((dir, local_link_kind)) = html_export {
        n.export_html(
            &LIB_CFG.read_recursive().tmpl_html.exporter,
            dir,
            *local_link_kind,
        )?;
    }

    // If no new filename was rendered, return the old one.
    let mut n = n;
    if n.rendered_filename == PathBuf::new() {
        n.rendered_filename = n.context.path.clone();
    }

    Ok(n.rendered_filename)
}

///
/// Helper function.
fn synchronize<T: Content>(
    settings: &mut RwLockUpgradableReadGuard<Settings>,
    note: &mut Note<T>,
) -> Result<(), NoteError> {
    let no_filename_sync = match (
        note.context.get(TMPL_VAR_FM_FILENAME_SYNC),
        note.context.get(TMPL_VAR_FM_NO_FILENAME_SYNC),
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
    match note.context.get(TMPL_VAR_FM_SCHEME).as_ref() {
        Some(Value::String(s)) if !s.is_empty() => {
            // Initialize `SETTINGS`.
            settings.with_upgraded(|settings| settings.update(SchemeSource::Force(s), None))?;
        }
        Some(Value::String(_)) | None => {
            // Initialize `SETTINGS`.
            settings
                .with_upgraded(|settings| settings.update(SchemeSource::SchemeSyncDefault, None))?;
        }
        Some(_) => {
            return Err(NoteError::FrontMatterFieldIsNotString {
                field_name: TMPL_VAR_FM_SCHEME.to_string(),
            });
        }
    };

    note.render_filename(TemplateKind::SyncFilename)?;

    note.set_next_unused_rendered_filename_or(&note.context.path.clone())?;
    // Silently fails is source and target are identical.
    note.rename_file_from(&note.context.path)?;

    Ok(())
}

/// Returns the HTML rendition of a `ContentString`. The markup rendition
/// engine is determined, by the file extension of the variable `context.path`.
/// The resulting HTML and other HTML template variables originating from
/// `context` are inserted into the `TMPL_HTML_VIEWER` template (which can be
/// replaced at runtime) before being returned. This function is stateless.
///
/// ```rust
/// use tpnote_lib::config::TMPL_HTML_VAR_VIEWER_DOC_JS;
/// use tpnote_lib::content::Content;
/// use tpnote_lib::content::ContentString;
/// use tpnote_lib::context::Context;
/// use tpnote_lib::workflow::render_viewer_html;
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
/// let html = render_viewer_html::<ContentString>(context, raw.into())
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
/// use tpnote_lib::workflow::render_viewer_html;
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
/// let html = render_viewer_html(context, content).unwrap();
/// // Check the HTML rendition.
/// assert!(html.starts_with("<!DOCTYPE html>\n<html"))
/// ```
pub fn render_viewer_html<T: Content>(context: Context, content: T) -> Result<String, NoteError> {
    let tmpl_html = &LIB_CFG.read_recursive().tmpl_html.viewer;
    render_html(context, content, tmpl_html)
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
/// use tpnote_lib::workflow::render_exporter_html;
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
/// let html = render_exporter_html::<ContentString>(context, raw.into())
///            .unwrap();
/// // Check the HTML rendition.
/// assert!(html.starts_with("<!DOCTYPE html>\n<html"))
/// ```
pub fn render_exporter_html<T: Content>(context: Context, content: T) -> Result<String, NoteError> {
    let tmpl_html = &LIB_CFG.read_recursive().tmpl_html.exporter;
    render_html(context, content, tmpl_html)
}

/// Helper function.
fn render_html<T: Content>(
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
/// and `TMPL_HTML_VAR_VIEWER_DOC_JS` and `TMPL_HTML_VAR_NOTE_ERROR` in
/// `context` to be set.
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
/// use tpnote_lib::workflow::render_erroneous_content_html;
/// use std::env::temp_dir;
/// use std::fs;
///
/// // Prepare test: create existing errorneous note file.
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
/// context.insert(TMPL_HTML_VAR_VIEWER_DOC_JS, &e.to_string());
/// // We simulate an error;
/// context.insert(TMPL_HTML_VAR_DOC_ERROR, &e.to_string());
/// // Render.
/// // Read from file.
/// // You can plug in your own type (must impl. `Content`).
/// let content = ContentString::open(&context.path).unwrap();
/// let html = render_erroneous_content_html(
///               context, content).unwrap();
/// // Check the HTML rendition.
/// assert!(html.starts_with("<!DOCTYPE html>\n<html"))
/// ```
#[cfg(feature = "viewer")]
pub fn render_erroneous_content_html<T: Content>(
    mut context: Context,
    note_erroneous_content: T,
) -> Result<String, NoteError> {
    // Insert.

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
