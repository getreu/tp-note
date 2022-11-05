//! Tp-Note's high level API. The low level API is documented
//! in the module `tpnote_lib::note`.
//!
//! How to integrate this in your text editor code?
//! First, call `create_new_note_or_synchronize_filename()`
//! with the first positional command line parameter `<path>`.
//! Then open the text file `<Note>.rendered_filename` in your
//! text editor or alternatively, load the string
//! `<Note>.content.as_str()` directly into your text editor.
//! After saving the text file, call `synchronize_filename()`
//! and update your file path with `<Note>.rendered_filename`.
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
//! let template_kind_filer = |tk|tk;
//!
//! // Start test.
//! // You can plug in your own type (must impl. `Content`).
//! let n = create_new_note_or_synchronize_filename::<ContentString, _>(
//!        &notedir, &clipboard, &stdin, template_kind_filer, None).unwrap();
//! // Check result.
//! assert!(n.rendered_filename.as_os_str().to_str().unwrap()
//!    .contains("--Note"));
//! assert!(n.rendered_filename.is_file());
//! let raw_note = fs::read_to_string(n.rendered_filename).unwrap();
//! assert!(raw_note.starts_with("\u{feff}---\ntitle:"));
//! ```
//!
//! The internal data storage for the note's content is `ContentString`
//! which implements the `Content` trait. Now we modify slightly  
//! the above example to showcases, how to overwrite
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
//! // We need a newtype because of the orphan.
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
//! let template_kind_filer = |tk|tk;
//!
//! // Start test.
//! // Here we plugin our own type (must implement `Content`).
//! let n = create_new_note_or_synchronize_filename::<MyContentString, _>(
//!        &notedir, &clipboard, &stdin, template_kind_filer, None).unwrap();
//! // Check result.
//! assert!(n.rendered_filename.as_os_str().to_str().unwrap()
//!    .contains("--Note"));
//! assert!(n.rendered_filename.is_file());
//! let raw_note = fs::read_to_string(n.rendered_filename).unwrap();
//! assert_eq!(raw_note, "Simulation");
//! ```

use crate::config::LIB_CFG;
use crate::config::TMPL_VAR_CLIPBOARD;
use crate::config::TMPL_VAR_CLIPBOARD_HEADER;
use crate::config::TMPL_VAR_FM_;
use crate::config::TMPL_VAR_FM_FILENAME_SYNC;
use crate::config::TMPL_VAR_FM_NO_FILENAME_SYNC;
use crate::config::TMPL_VAR_STDIN;
use crate::config::TMPL_VAR_STDIN_HEADER;
use crate::content::Content;
use crate::context::Context;
use crate::error::NoteError;
use crate::note::Note;
use crate::template::TemplateKind;
use std::path::Path;
use std::path::PathBuf;
use tera::Value;

/// Open the note file `path` on disk and read its YAML front matter.
/// Then calculate from the front matter how the filename should be to
/// be in sync. If it is different, rename the note on disk.
/// Returns the note's new or existing filename in `<Note>.rendered_filename`.
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
/// let res_fn = n.rendered_filename;
///
/// // Check result
/// assert_eq!(res_fn, expected);
/// assert!(res_fn.is_file());
/// let res_raw = fs::read_to_string(&res_fn).unwrap();
/// assert_eq!(res_raw, raw);
/// ```
pub fn synchronize_filename<T: Content>(path: &Path) -> Result<Note<T>, NoteError> {
    // Collect input data for templates.
    let mut context = Context::from(&path);
    context.insert_environment()?;

    let content = <T>::open(&path).unwrap_or_default();
    let n = synchronize::<T>(context, content)?;

    Ok(n)
}

#[inline]
/// Create a new note by inserting `Tp-Note`'s environment in a template.
/// If the note to be created exists already, append a so called `copy_counter`
/// to the filename and try to save it again. In case this does not succeed either,
/// increment the `copy_counter` until a free filename is found.
/// The return path in `<Note>.rendered_filename` points to the (new) note file on disk.
/// Depending on the context, Tp-Note chooses one `TemplateKind` to operate
/// (c.f. `tpnote_lib::template::TemplateKind::from()`).
/// The `tk-filter` allows to overwrite this choice, e.g. you may set
/// `TemplateKind::None` under certain circumstances. This way the caller
/// can inject command line parameters like `--no-filename-sync`.
///
/// Returns the note's new or existing filename in `<Note>.rendered_filename`.
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
/// let template_kind_filer = |tk|tk;
///
/// // Start test.
/// // You can plug in your own type (must impl. `Content`).
/// let n = create_new_note_or_synchronize_filename::<ContentString, _>(
///        &notedir, &clipboard, &stdin, template_kind_filer, None).unwrap();
/// // Check result.
/// assert!(n.rendered_filename.as_os_str().to_str().unwrap()
///    .contains("my stdin-my clipboard--Note"));
/// assert!(n.rendered_filename.is_file());
/// let raw_note = fs::read_to_string(n.rendered_filename).unwrap();
/// assert!(raw_note.starts_with("\u{feff}---\ntitle:      \"my stdin\\nmy clipboard\\n\""));
/// ```
pub fn create_new_note_or_synchronize_filename<T, F>(
    path: &Path,
    clipboard: &T,
    stdin: &T,
    tk_filter: F,
    args_export: Option<&Path>,
) -> Result<Note<T>, NoteError>
where
    T: Content,
    F: Fn(TemplateKind) -> TemplateKind,
{
    // First, generate a new note (if it does not exist), then parse its front_matter
    // and finally rename the file, if it is not in sync with its front matter.

    // Collect input data for templates.
    let mut context = Context::from(path);
    context.insert_environment()?;
    context.insert_content(TMPL_VAR_CLIPBOARD, TMPL_VAR_CLIPBOARD_HEADER, clipboard)?;
    context.insert_content(TMPL_VAR_STDIN, TMPL_VAR_STDIN_HEADER, stdin)?;

    // `template_king` will tell us what to do.
    let (template_kind, content) = TemplateKind::from::<T>(path, clipboard, stdin);
    let template_kind = tk_filter(template_kind);

    let n = match template_kind {
        TemplateKind::New
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
            let mut n = Note::from_text_file(context, content.unwrap(), template_kind)?;
            // Render filename.
            n.render_filename(template_kind)?;

            // Save new note.
            let context_path = n.context.path.clone();
            n.set_next_unused_rendered_filename_or(&context_path)?;
            n.save_and_delete_from(&context_path)?;
            n
        }
        TemplateKind::SyncFilename => synchronize(context, content.unwrap())?,
        TemplateKind::None => Note::from_text_file(context, content.unwrap(), template_kind)?,
    };

    // Export HTML rendition, if wanted.
    if let Some(dir) = args_export {
        n.export_html(&LIB_CFG.read().unwrap().tmpl_html.exporter, dir)?;
    }

    // If no new filename was rendered, return the old one.
    let mut n = n;
    if n.rendered_filename == PathBuf::new() {
        n.rendered_filename = n.context.path.clone();
    }

    Ok(n)
}

/// Helper function.
fn synchronize<T: Content>(context: Context, content: T) -> Result<Note<T>, NoteError> {
    // parse file again to check for synchronicity with filename

    let mut n = Note::from_text_file(context, content, TemplateKind::SyncFilename)?;

    let no_filename_sync = match (
        n.context.get(TMPL_VAR_FM_FILENAME_SYNC),
        n.context.get(TMPL_VAR_FM_NO_FILENAME_SYNC),
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
    } else {
        n.render_filename(TemplateKind::SyncFilename)?;

        n.set_next_unused_rendered_filename_or(&n.context.path.clone())?;
        // Silently fails is source and target are identical.
        n.rename_file_from(&n.context.path)?;
    }

    Ok(n)
}
