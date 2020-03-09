//! Creates a memory representations of the note by inserting `tp-note`'s
//! environment data in some templates. If the note exists on disk already,
//! the memory representation is established be reading the note-file with
//! its front matter.

extern crate chrono;
extern crate tera;
extern crate time;

use crate::config::Hyperlink;
use crate::config::ARGS;
use crate::config::CFG;
use crate::config::CLIPBOARD;
use crate::content::Content;
use crate::context::ContextWrapper;
use anyhow::{anyhow, Context, Result};
use serde::{Deserialize, Serialize};
use std::default::Default;
use std::env;
use std::path::{Path, PathBuf};
use tera::Tera;

#[derive(Debug, PartialEq)]
/// Represents a note.
pub struct Note {
    /// The front matter of the note.
    front_matter: Option<FrontMatter>,
    /// Captured environment of `tp-note` that
    /// is used to fill in templates.
    context: ContextWrapper,
    /// The full text content of the note, including
    /// its front matter.
    pub content: Content,
}

#[derive(Debug, PartialEq, Serialize, Deserialize, Default)]
/// Represents the front matter of the note.
struct FrontMatter {
    /// The compulsory note's title.
    title: String,
    /// The compulsory note's subtitle.
    subtitle: String,
}

use std::fs;
impl Note {
    /// Constructor that creates a memory representation of an existing note on
    /// disk.
    pub fn from_existing_note(path: &Path) -> Result<Self> {
        let content = Content::new(
            fs::read_to_string(path)
                .with_context(|| format!("failed to read `{}`", path.display()))?
                .as_str(),
        );
        let fm = Self::deserialize_note(&content)?;

        let mut context = Self::capture_environment(&path)?;

        context.insert("title", &fm.title);
        context.insert("subtitle", &fm.subtitle);

        Ok(Self {
            front_matter: Some(fm),
            context,
            content,
        })
    }

    /// Constructor that creates a new note by filling in the template
    /// `template`.
    pub fn new(path: &Path, template: &str) -> Result<Self> {
        // render template

        // there is no front matter yet to capture
        let mut context = Self::capture_environment(&path)?;

        let content = Content::new(
            Tera::one_off(template, &context, false)
                .with_context(|| format!("failed to render template:\n`{}`", template))?
                .as_str(),
        );

        // deserialize the rendered result
        let fm = Note::deserialize_note(&content)?;

        context.insert("title", &fm.title);
        context.insert("subtitle", &fm.subtitle);

        if ARGS.debug {
            eprintln!("*** Content template used:\n{}\n\n", template);
        };

        // return result
        Ok(Self {
            front_matter: Some(fm),
            context,
            content,
        })
    }

    /// Capture `tp_note`'s environment and stores it as variables in a
    /// `context` collection. The variables are needed later to populate
    /// a context-template and a filename-template.
    /// The `path` parameter must be a canonicalized fully qualified file name.
    fn capture_environment(path: &Path) -> Result<ContextWrapper> {
        let mut context = ContextWrapper::new();

        let sort_tag: String = path
            .file_stem()
            .unwrap_or_default()
            .to_str()
            .unwrap_or_default()
            .chars()
            .take_while(|&c| c.is_numeric() || c == '-' || c == '_')
            .collect::<String>();
        context.insert("sort_tag", &sort_tag);

        // `fqpn` is a directory as fully qualified path, ending
        // by a separator.
        let fqpn = if path.is_dir() {
            path
        } else {
            path.parent().unwrap_or(Path::new("./"))
        };
        context.insert("path", &fqpn.to_str().unwrap_or_default());

        // Strip off the sort tag if there is any
        let dirname = fqpn
            .file_name()
            .unwrap_or_default()
            .to_str()
            .unwrap_or_default()
            .trim_start_matches(|c: char| c.is_numeric() || c == '-' || c == '_');
        context.insert("dirname", &dirname);

        // Strip off the sort tag if there is any.
        let file_stem = if path.is_dir() {
            ""
        } else {
            path.file_stem()
                .unwrap_or_default()
                .to_str()
                .unwrap_or_default()
                .trim_start_matches(|c: char| c.is_numeric() || c == '-' || c == '_')
        };
        context.insert("file_stem", &file_stem);

        let clipboard: String = CLIPBOARD.as_ref().unwrap_or(&String::default()).to_string();
        context.insert("clipboard", &clipboard);

        // parse clipboard
        let hyperlink = match Hyperlink::new(&clipboard) {
            Ok(s) => Some(s),
            Err(e) => {
                if ARGS.debug {
                    eprintln!("Note: clipboard does not contain a markdown link: {}", e);
                }
                None
            }
        };

        // register clipboard
        context.insert("clipboard", &clipboard);
        // if there is a hyperlink register it too
        if hyperlink.is_some() {
            context.insert("clipboard_linkname", &(&hyperlink).as_ref().unwrap().name);
            context.insert("clipboard_linkurl", &(&hyperlink).as_ref().unwrap().url);
        } else {
            context.insert("clipboard_linkname", &"");
            context.insert("clipboard_linkurl", &"");
        };

        // extension of the path given on command-line
        context.insert(
            "extension",
            &path
                .extension()
                .unwrap_or_default()
                .to_str()
                .unwrap_or_default(),
        );
        context.insert("note_extension", CFG.note_extension.as_str());

        // search for UNIX or Windows user-names
        let author = env::var("LOGNAME").unwrap_or(env::var("USERNAME").unwrap_or_default());
        context.insert("username", &author);

        // register locale if available
        let lang = env::var("LANG").unwrap_or_default();
        context.insert("lang", &lang);

        // register all environment variables for usage in template
        for (key, value) in env::vars() {
            context.insert(&key, &value);
        }

        context.fqpn = fqpn.to_path_buf();

        Ok(context)
    }

    /// Applies a Tera-template to the notes context in order to generate a
    /// sanitized filename that is in sync with the note's meta data stored in
    /// its front matter.
    pub fn render_filename(&self, template: &str) -> Result<PathBuf> {
        if ARGS.debug {
            eprintln!("*** Filename template used:\n{}\n\n", template);
            eprintln!(
                "*** Substitution variables for filename template:\n{:#?}",
                &self.context
            );
        }

        // render template
        let mut fqfn = self.context.fqpn.to_owned();
        fqfn.push(
            Tera::one_off(template, &self.context, false)
                .with_context(|| format!("failed to render template:\n`{}`", template))?
                .trim(),
        );

        Ok(Path::new(&fqfn).to_path_buf())
    }

    /// Helper function deserializing the front-matter of an `.md`-file.
    fn deserialize_note(content: &str) -> Result<FrontMatter> {
        // anyhow Error type

        let fm_start = content
            .find("---")
            .context("no YAML front matter start line '---' found")?
            + 3;

        let fm_end = content[fm_start..]
            .find("---\n")
            .unwrap_or(content[fm_start..].find("...\n").unwrap_or(0))
            + fm_start;

        if fm_start >= fm_end {
            return Err(anyhow!(
                "no YAML front matter end line `---` or `...` found"
            ));
        }

        let fm: FrontMatter = serde_yaml::from_str(&content[fm_start..fm_end])?;
        Ok(fm)
    }
}

#[cfg(test)]
mod tests {
    use super::FrontMatter;
    use super::Note;

    #[test]
    fn test_from_existing_note() {
        // TODO add test
    }

    #[test]
    fn test_new_note() {
        // TODO add test
    }

    #[test]
    fn test_deserialize() {
        let input = "--- # document start
        title: The book
        subtitle: you always wanted
        author: Is's me
        ...\ncontent\nmore content";

        let expected_front_matter = FrontMatter {
            title: "The book".to_string(),
            subtitle: "you always wanted".to_string(),
        };

        assert_eq!(
            expected_front_matter,
            Note::deserialize_note(&input).unwrap()
        );

        // Front matter can also end with '---'

        let input = "--- # document start
        title: \"The book\"
        subtitle: you always wanted
        author: It's me
        ---\ncontent\nmore content";

        let expected_front_matter = FrontMatter {
            title: "The book".to_string(),
            subtitle: "you always wanted".to_string(),
        };

        assert_eq!(
            expected_front_matter,
            Note::deserialize_note(&input).unwrap()
        );

        // Missing start '---'

        let input = "-- # document start
        title: The book
        subtitle: you always wanted
        author: Is's me
        ...\ncontent\nmore content";

        assert!(Note::deserialize_note(&input).is_err());

        // Missing end '...'

        let input = "--- # document start
        title: The book
        subtitle: you always wanted
        author: It's me
        ..\ncontent\nmore content";

        assert!(Note::deserialize_note(&input).is_err());

        // Missing title

        let input = "--- # document start
        titlxxx: The book
        subtitle: you always wanted
        author: It's me
        ...\ncontent\nmore content";

        assert!(Note::deserialize_note(&input).is_err());

        // Missing subtitle

        let input = "--- # document start
        title: The book
        subtitlxxx: you always wanted
        author: It's me
        ...\ncontent\nmore content";

        assert!(Note::deserialize_note(&input).is_err());
    }
}
