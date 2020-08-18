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
    /// The compulsory note's title.
    title: String,
    /// The compulsory note's subtitle.
    subtitle: String,
    /// Optional tag variable. If not defined in front matter,
    /// the file name's sort tag is used (if any).
    tag: Option<String>,
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

        // Copy value of `file_tag` as default for `tag`.
        if let Some(val) = context.get("file_tag") {
            if let serde_json::Value::String(file_tag) = val {
                let file_tag = file_tag.to_string();
                context.insert("tag", &file_tag);
            } else {
                context.insert("tag", "");
            }
        }

        // Read YAML header variable `tag` if any.
        if let Some(tag) = &fm.tag {
            if !tag.is_empty() {
                // Overwrites `tag` key copied above.
                context.insert("tag", tag);
            };
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
                .with_context(|| format!("failed to render template:\n`{}`", template))?
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
        context.insert("subtitle", &fm.subtitle);

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

        let file_tag: String = path
            .file_stem()
            .unwrap_or_default()
            .to_str()
            .unwrap_or_default()
            .chars()
            .take_while(|&c| c.is_numeric() || c == '-' || c == '_')
            .collect::<String>();
        // Sort-tag that is deduced from filename on disk.
        context.insert("file_tag", &file_tag);

        // `fqpn` is a directory as fully qualified path, ending
        // by a separator.
        let fqpn = if path.is_dir() {
            path
        } else {
            path.parent().unwrap_or_else(|| Path::new("./"))
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

        // Register input from clipboard.
        context.insert("clipboard", &CLIPBOARD.content);
        context.insert("clipboard_truncated", &CLIPBOARD.content_truncated);
        context.insert("clipboard_heading", &CLIPBOARD.content_heading);
        context.insert("clipboard_linkname", &CLIPBOARD.linkname);
        context.insert("clipboard_linkurl", &CLIPBOARD.linkurl);

        // Extension of the path given on command-line.
        context.insert(
            "extension",
            &path
                .extension()
                .unwrap_or_default()
                .to_str()
                .unwrap_or_default(),
        );
        context.insert("note_extension", CFG.note_extension.as_str());

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
                .with_context(|| format!("failed to render template:\n`{}`", template))?
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
            .context("no YAML front matter start line '---' found")?
            + 3;

        let fm_end = content[fm_start..]
            .find("---\n")
            .unwrap_or_else(|| content[fm_start..].find("...\n").unwrap_or(0))
            + fm_start;

        if fm_start >= fm_end {
            return Err(anyhow!(
                "no YAML front matter end line `---` or `...` found"
            ));
        }

        let fm: FrontMatter = serde_yaml::from_str(&content[fm_start..fm_end])?;

        // `tag` has additional constrains to check.
        if let Some(tag) = &fm.tag {
            if !tag.is_empty() {
                // Check for forbidden characters.
                if tag
                    .chars()
                    .filter(|&c| !c.is_numeric() && c != '_' && c != '-')
                    .count()
                    > 0
                {
                    return Err(anyhow!(format!(
                        "Forbidden character(s) in `tag` \"{}\" variable. \
                     Only `0..9_-` are allowed.",
                        tag
                    )));
                }
            };
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
                println!("Creating file: {:?}", new_fqfn);
                write!(outfile, "{}", &self.content.to_osstring())
                    .with_context(|| format!("can not write new file {:?}", new_fqfn))?
            }
            Err(e) => {
                if Path::new(&new_fqfn).exists() {
                    println!("can not create new file, file exists: {}", e);
                    println!("Instead, try to read existing: {:?}", new_fqfn);
                } else {
                    return Err(anyhow!(format!(
                        "can not write file: {:?}\n{}",
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
            subtitle: "you always wanted".to_string(),
            tag: None,
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
            tag: None,
        };

        assert_eq!(
            expected_front_matter,
            Note::deserialize_note(&input).unwrap()
        );

        // Front matter can optionally have a tag

        let input = "--- # document start
        title: \"The book\"
        subtitle: you always wanted
        author: It's me
        tag: 20200420-21_22
        ---\ncontent\nmore content";

        let expected_front_matter = FrontMatter {
            title: "The book".to_string(),
            subtitle: "you always wanted".to_string(),
            tag: Some("20200420-21_22".to_string()),
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

        // Leading/trailing `_` or `-` in `tag`

        let input = "--- # document start
        title: The book
        subtitle: you always wanted
        author: It's me
        tag:    -1234
        ...\ncontent\nmore content";

        assert!(Note::deserialize_note(&input).is_err());

        // forbidden character `x` in `tag`.

        let input = "--- # document start
        title: The book
        subtitle: you always wanted
        author: It's me
        tag:    123x4
        ...\ncontent\nmore content";

        assert!(Note::deserialize_note(&input).is_err());
    }
}
