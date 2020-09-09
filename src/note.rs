//! Creates a memory representations of the note by inserting `tp-note`'s
//! environment data in some templates. If the note exists on disk already,
//! the memory representation is established be reading the note-file with
//! its front matter.

extern crate chrono;
extern crate tera;
extern crate time;

use crate::config::ARGS;
use crate::config::CFG;
use crate::config::CLIPBOARD;
use crate::config::NOTE_FILENAME_LEN_MAX;
use crate::config::STDIN;
use crate::content::Content;
use crate::filter::ContextWrapper;
use crate::filter::TERA;
use anyhow::{anyhow, Context, Result};
use serde::{Deserialize, Serialize};
use std::default::Default;
use std::env;
use std::fs::OpenOptions;
use std::io::Write;
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
    /// The note's compulsory title.
    title: String,
    /// The note's optional subtitle.
    subtitle: Option<String>,
    /// Optional YAML header variable. If not defined in front matter,
    /// the file name's sort tag `file | tag` is used (if any).
    sort_tag: Option<String>,
    /// Optional YAML header variable. If not defined in front matter,
    /// the file name's extension `file | extension` is used.
    extension: Option<String>,
}

use std::fs;
impl Note {
    /// Constructor that creates a memory representation of an existing note on
    /// disk.
    pub fn from_existing_note(path: &Path) -> Result<Self> {
        let content = Content::new(
            fs::read_to_string(path)
                .with_context(|| format!("Failed to read `{}`.", path.display()))?
                .as_str(),
        );
        let fm = Self::deserialize_note(&content)?;

        let mut context = Self::capture_environment(&path)?;

        context.insert("title", &fm.title);

        // Read YAML header variable `subtitle`, register an empty string if not defined.
        context.insert("subtitle", &fm.subtitle.as_ref().unwrap_or(&String::new()));

        // Read YAML header variable `extension` if any.
        if let Some(e) = &fm.extension {
            context.insert("extension", e);
        };

        // Read YAML header variable `tag` if any.
        if let Some(st) = &fm.sort_tag {
            context.insert("sort_tag", st);
        };

        Ok(Self {
            front_matter: Some(fm),
            context,
            content,
        })
    }

    /// Constructor that creates a new note by filling in the content template `template`.  The
    /// newly created file will never be saved with TMPL_SYNC_FILENAME.  As the latter is the only
    /// filename template that is allowed to use the variable `tag`, we do not need to insert it
    /// here.
    pub fn new(path: &Path, template: &str) -> Result<Self> {
        // render template

        // there is no front matter yet to capture
        let mut context = Self::capture_environment(&path)?;

        let content = Content::new({
            let mut tera = Tera::default();
            tera.extend(&TERA).unwrap();

            tera.render_str(template, &context)
                .with_context(|| format!("Failed to render the template:\n`{}`.", template))?
                .as_str()
        });

        if ARGS.debug {
            eprintln!(
                "*** Debug: Available substitution variables for context template:\n{:#?}\n",
                *context
            );
            eprintln!("*** Debug: Applying content template:\n{}\n", template);
            eprintln!("*** Debug: Rendered content template:\n{}\n\n", *content);
        };

        // deserialize the rendered result
        let fm = Note::deserialize_note(&content)?;

        context.insert("title", &fm.title);
        context.insert("subtitle", &fm.subtitle.as_ref().unwrap_or(&String::new()));

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

        // Register the canonicalized fully qualified file name.
        let file = path.to_str().unwrap_or_default();
        context.insert("file", &file);

        // `fqpn` is a directory as fully qualified path, ending
        // by a separator.
        let fqpn = if path.is_dir() {
            path
        } else {
            path.parent().unwrap_or_else(|| Path::new("./"))
        };
        context.insert("path", &fqpn.to_str().unwrap_or_default());

        // Register input from clipboard.
        context.insert("clipboard", &CLIPBOARD);

        // Register input from stdin.
        context.insert("stdin", &STDIN);

        // Default extension for new notes as defined in the configuration file.
        context.insert("extension_default", CFG.extension_default.as_str());

        // search for UNIX, Windows and MacOS user-names
        let author = env::var("LOGNAME").unwrap_or_else(|_| {
            env::var("USERNAME").unwrap_or_else(|_| env::var("USER").unwrap_or_default())
        });
        context.insert("username", &author);

        context.fqpn = fqpn.to_path_buf();

        Ok(context)
    }

    /// Applies a Tera-template to the notes context in order to generate a
    /// sanitized filename that is in sync with the note's meta data stored in
    /// its front matter.
    pub fn render_filename(&self, template: &str) -> Result<PathBuf> {
        if ARGS.debug {
            eprintln!(
                "*** Debug: Available substitution variables for filename template:\n{:#?}\n",
                *self.context
            );
            eprintln!("*** Debug: Applying filename template:\n{}\n\n", template);
        };

        // render template
        let mut fqfn = self.context.fqpn.to_owned();
        fqfn.push({
            let mut tera = Tera::default();
            tera.extend(&TERA).unwrap();

            tera.render_str(template, &self.context)
                .map(|filename| {
                    if ARGS.debug {
                        eprintln!("*** Debug: Rendered filename template:\n{:?}\n\n", filename);
                    };
                    filename
                })
                .with_context(|| format!("Failed to render the template:\n`{}`.", template))?
                .trim()
        });

        Ok(Self::shorten_filename(Path::new(&fqfn)))
    }

    /// Shortens the stem of a filename so that
    /// `file_stem.len()+file_extension.len() <= NOTE_FILENAME_LEN_MAX`
    fn shorten_filename(fqfn: &Path) -> PathBuf {
        let mut parent = if let Some(p) = fqfn.parent() {
            p.to_path_buf()
        } else {
            PathBuf::new()
        };
        // Determine length of file-extension.
        let mut note_extension_len = 0;
        let mut note_extension = "";
        if let Some(ext) = &fqfn.extension() {
            if let Some(ext) = ext.to_str() {
                note_extension_len = ext.len();
                note_extension = ext;
            }
        };

        // Limit length of file-stem.
        let mut note_stem_short = String::new();
        if let Some(note_stem) = &fqfn.file_stem() {
            if let Some(note_stem) = note_stem.to_str() {
                // Limit the size of `fqfn`
                // `+1` reserves one byte for `.` before the extension.
                for i in (0..NOTE_FILENAME_LEN_MAX - (note_extension_len + 1)).rev() {
                    if let Some(s) = note_stem.get(..=i) {
                        note_stem_short = s.to_string();
                        break;
                    }
                }
            }
        };

        // Assemble.
        let mut note_filename = note_stem_short;
        note_filename.push('.');
        note_filename.push_str(note_extension);

        // Add to parent.
        parent.push(Path::new(&note_filename).to_path_buf());
        parent
    }

    /// Helper function deserializing the front-matter of an `.md`-file.
    fn deserialize_note(content: &str) -> Result<FrontMatter> {
        // anyhow Error type

        let fm_start = content
            .find("---")
            .context("No YAML front matter start line '---' found.")?
            + 3;

        let fm_end = content[fm_start..]
            .find("---\n")
            .unwrap_or_else(|| content[fm_start..].find("...\n").unwrap_or(0))
            + fm_start;

        if fm_start >= fm_end {
            return Err(anyhow!(
                "No YAML front matter end line `---` or `...` found."
            ));
        }

        let fm: FrontMatter = serde_yaml::from_str(&content[fm_start..fm_end])?;

        // `tag` has additional constrains to check.
        if let Some(sort_tag) = &fm.sort_tag {
            if !sort_tag.is_empty() {
                // Check for forbidden characters.
                if sort_tag
                    .chars()
                    .filter(|&c| !c.is_numeric() && c != '_' && c != '-')
                    .count()
                    > 0
                {
                    return Err(anyhow!(format!(
                        "The `tag`-variable contains forbidden character(s): tag = \"{}\". \
                        Only numbers, `-` and `_` are allowed here.",
                        sort_tag
                    )));
                }
            };
        };

        // `extension` has also additional constrains to check.
        // Is `extension` listed in `CFG.note_file_extension`?
        if let Some(extension) = &fm.extension {
            let mut extension_is_known = false;
            for e in &CFG.note_file_extensions {
                if e == extension {
                    extension_is_known = true;
                    break;
                }
            }
            if !extension_is_known {
                return Err(anyhow!(format!(
                    "`extension=\"{}\"`, is not registered as a valid\n\
                        Tp-Note-file in the `note_file_extensions` variable\n\
                        in your configuration file:\n\
                        \t{:?}\n\
                        \n\
                        Choose one of the above list or add more extensions to\n\
                        `note_file_extensions` in your configuration file.",
                    extension, &CFG.note_file_extensions
                )));
            }
        };

        Ok(fm)
    }

    /// Writes the note to disk with `new_fqfn`-filename.
    pub fn write_to_disk(&self, new_fqfn: &Path) -> Result<PathBuf, anyhow::Error> {
        // Write new note on disk.
        let outfile = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&new_fqfn);
        match outfile {
            Ok(mut outfile) => {
                if ARGS.debug {
                    eprintln!("Creating file: {:?}", new_fqfn);
                };
                write!(outfile, "{}", &self.content.to_osstring())
                    .with_context(|| format!("Can not write new file {:?}", new_fqfn))?
            }
            Err(e) => {
                if Path::new(&new_fqfn).exists() {
                    if ARGS.debug {
                        eprintln!("Info: Can not create new file, file exists: {}", e);
                        eprintln!("Info: Instead, try to read existing: {:?}", new_fqfn);
                    };
                } else {
                    return Err(anyhow!(format!(
                        "Can not write file: {:?}\n{}",
                        new_fqfn, e
                    )));
                }
            }
        }

        Ok(new_fqfn.to_owned())
    }
}

#[cfg(test)]
mod tests {
    use super::FrontMatter;
    use super::Note;

    #[test]
    fn test_shorten_filename() {
        use std::ffi::OsString;
        use std::path::Path;

        // Test short filename.
        let input = Path::new("long directory name/abc.ext");
        let output = Note::shorten_filename(input);
        assert_eq!(OsString::from("long directory name/abc.ext"), output);

        // Test long filename.
        let input = Path::new("long directory name/long filename.ext");
        let output = Note::shorten_filename(input);
        assert_eq!(OsString::from("long directory name/long f.ext"), output);
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
            subtitle: Some("you always wanted".to_string()),
            sort_tag: None,
            extension: None,
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
            subtitle: Some("you always wanted".to_string()),
            sort_tag: None,
            extension: None,
        };

        assert_eq!(
            expected_front_matter,
            Note::deserialize_note(&input).unwrap()
        );

        // Front matter can optionally have a tag and an extension

        let input = "--- # document start
        title: \"The book\"
        subtitle: you always wanted
        author: It's me
        sort_tag: 20200420-21_22
        extension: md
        ---\ncontent\nmore content";

        let expected_front_matter = FrontMatter {
            title: "The book".to_string(),
            subtitle: Some("you always wanted".to_string()),
            sort_tag: Some("20200420-21_22".to_string()),
            extension: Some("md".to_string()),
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

        let expected_front_matter = FrontMatter {
            title: "The book".to_string(),
            subtitle: None,
            sort_tag: None,
            extension: None,
        };

        assert_eq!(
            expected_front_matter,
            Note::deserialize_note(&input).unwrap()
        );

        // forbidden character `x` in `tag`.

        let input = "--- # document start
        title: The book
        subtitle: you always wanted
        author: It's me
        sort_tag:    123x4
        ...\ncontent\nmore content";

        assert!(Note::deserialize_note(&input).is_err());
    }
}
