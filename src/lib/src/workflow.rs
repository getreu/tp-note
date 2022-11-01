//! High level API. TODO
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
/// be in sync. If it is different, rename the note on disk and return
/// the new filename in `note.rendered_filename`.
/// If no filename was rendered, `note.rendered_filename == PathBuf::new()`
pub fn synchronize_filename<T: Content>(
    context: Context,
    content: T,
) -> Result<Note<T>, NoteError> {
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

#[inline]
/// Create a new note by inserting `Tp-Note`'s environment in a template.
/// If the note to be created exists already, append a so called `copy_counter`
/// to the filename and try to save it again. In case this does not succeed either,
/// increment the `copy_counter` until a free filename is found.
/// The return path points to the (new) note file on disk.
/// If an existing note file was not moved, the return path equals to `context.path`.
///
pub fn create_new_note_or_synchronize_filename<T, F>(
    path: &Path,
    clipboard: &T,
    stdin: &T,
    tk_filter: F,
    args_export: Option<&Path>,
) -> Result<PathBuf, NoteError>
where
    T: Content,
    F: Fn(TemplateKind) -> TemplateKind,
{
    // First generate a new note (if it does not exist), then parse its front_matter
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
        TemplateKind::SyncFilename => synchronize_filename(context, content.unwrap())?,
        TemplateKind::None => Note::from_text_file(context, content.unwrap(), template_kind)?,
        #[allow(unreachable_patterns)]
        _ =>
        // Early return, we do nothing here and continue.
        {
            return Ok(context.path)
        }
    };

    // Export HTML rendition, if wanted.
    if let Some(dir) = args_export {
        n.export_html(&LIB_CFG.read().unwrap().tmpl_html.exporter, dir)?;
    }

    // If no new filename was rendered, return the old one.
    if n.rendered_filename != PathBuf::new() {
        Ok(n.rendered_filename)
    } else {
        Ok(n.context.path)
    }
}
