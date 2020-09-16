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
use std::matches;
use std::path::{Path, PathBuf};
use tera::Tera;

#[derive(Debug, PartialEq)]
/// Represents a note.
pub struct Note {
    // Reserved for future use:
    //     /// The front matter of the note.
    //     front_matter: FrontMatter,
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
    /// Optional YAML header variable.
    author: Option<String>,
    /// Optional YAML header variable.
    date: Option<String>,
    /// Optional YAML header variable.
    lang: Option<String>,
    /// Optional YAML header variable.
    revision: Option<String>,
    /// Optional YAML header variable. If not defined in front matter,
    /// the file name's sort tag `file | tag` is used.
    sort_tag: Option<String>,
    /// Optional YAML header variable. If not defined in front matter,
    /// the file name's extension `file | ext` is used.
    file_ext: Option<String>,
}

use std::fs;
impl Note {
    /// Constructor that creates a memory representation of an existing note on
    /// disk.
    pub fn from_existing_note(path: &Path) -> Result<Self> {
        let content = Content::new_relax(
            fs::read_to_string(path)
                .with_context(|| format!("Failed to read `{}`.", path.display()))?,
        );

        let mut context = Self::capture_environment(&path)?;

        // deserialize the note read from disk
        let fm = Note::deserialize_note(&content.get_header())?;

        Self::register_front_matter(&mut context, &fm);

        // Return new note.
        Ok(Self {
            // Reserved for future use:
            //     front_matter: fm,
            context,
            content,
        })
    }

    /// Constructor that creates a new note by filling in the content template `template`.
    pub fn from_content_template(path: &Path, template: &str) -> Result<Self> {
        let mut context = Self::capture_environment(&path)?;

        // render template
        let content = Content::new_relax({
            let mut tera = Tera::default();
            tera.extend(&TERA).unwrap();

            tera.render_str(template, &context)
                .with_context(|| format!("Failed to render the template:\n`{}`.", template))?
        });

        if ARGS.debug {
            eprintln!(
                "*** Debug: Available substitution variables for content template:\n{:#?}\n",
                *context
            );
            eprintln!("*** Debug: Applying content template:\n{}\n", template);
            eprintln!(
                "*** Debug: Rendered content template:\n---\n{}\n---\n{}\n\n",
                content.get_header(),
                content.get_body_or_text().trim()
            );
        };

        // deserialize the rendered template
        let fm = Note::deserialize_note(&content.get_header())?;

        Self::register_front_matter(&mut context, &fm);

        // Return new note.
        Ok(Self {
            // Reserved for future use:
            //     front_matter: fm,
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
        context.insert("clipboard_header", &*CLIPBOARD.get_header());
        context.insert("clipboard", &*CLIPBOARD.get_body_or_text());

        // Register input from stdin.
        context.insert("stdin_header", &*STDIN.get_header());
        context.insert("stdin", &*STDIN.get_body_or_text());

        // Can we find a front matter in the input stream? If yes, the
        // unmodified input stream is our new note content.
        let stdin_fm = Self::deserialize_note(&*STDIN.get_header()).ok();
        if ARGS.debug && stdin_fm.is_some() {
            eprintln!(
                "*** Debug: YAML front matter in the input stream stdin found:\n{:#?}",
                stdin_fm
            );
        };

        // Can we find a front matter in the clipboard? If yes, the unmodified
        // clipboard data is our new note content.
        let clipboard_fm = Self::deserialize_note(&*CLIPBOARD.get_header()).ok();
        if ARGS.debug && clipboard_fm.is_some() {
            eprintln!(
                "*** Debug: YAML front matter in the clipboard found:\n{:#?}",
                clipboard_fm
            );
        };

        if (matches!(*CLIPBOARD, Content::HeaderAndBody{..}) && clipboard_fm.is_none())
            || (matches!(*STDIN, Content::HeaderAndBody{..}) && stdin_fm.is_none())
        {
            return Err(anyhow!(format!(
                "no field `title: \"<String>\"` in the clipboard's YAML\n\
                     header or in the `stdin` input stream found.
                     {}{}{}{}{}{}",
                if matches!(*CLIPBOARD, Content::HeaderAndBody{..}) {
                    "\n*   Clipboard header:\n---\n"
                } else {
                    ""
                },
                CLIPBOARD.get_header(),
                if matches!(*CLIPBOARD, Content::HeaderAndBody{..}) {
                    "\n---"
                } else {
                    ""
                },
                if matches!(*STDIN, Content::HeaderAndBody{..}) {
                    "\n*   Input stream header:\n---\n"
                } else {
                    ""
                },
                STDIN.get_header(),
                if matches!(*STDIN, Content::HeaderAndBody{..}) {
                    "\n---"
                } else {
                    ""
                },
            )));
        };

        // Register stdin front matter.
        // Variables can be overwrittern by clipboard frontmatter.
        if let Some(fm) = stdin_fm {
            // Register YAML header variable `title`. We know, it exists.
            Self::register_front_matter(&mut context, &fm);
        }

        // Register clipboard front matter.
        if let Some(fm) = clipboard_fm {
            // Register YAML header variable `title`. We know, it exists.
            Self::register_front_matter(&mut context, &fm);
        }

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

    /// Copies the YAML front header variable in the context for later use with templates.
    fn register_front_matter(context: &mut ContextWrapper, fm: &FrontMatter) {
        context.insert("fm_title", &fm.title);

        // Read YAML header variable `subtitle`. If not defined, register nothing.
        if let Some(e) = &fm.subtitle {
            context.insert("fm_subtitle", e);
        };

        // Read YAML header variable `author`. If not defined, register nothing.
        if let Some(e) = &fm.author {
            context.insert("fm_author", e);
        };

        // Read YAML header variable `date`. If not defined, register nothing.
        if let Some(e) = &fm.date {
            context.insert("fm_date", e);
        };

        // Read YAML header variable `lang`. If not defined, register nothing.
        if let Some(e) = &fm.lang {
            context.insert("fm_lang", e);
        };

        // Read YAML header variable `revision`. If not defined, register nothing.
        if let Some(e) = &fm.revision {
            context.insert("fm_revision", e);
        };

        // Read YAML header variable `file_ext`. If not defined, register nothing.
        if let Some(e) = &fm.file_ext {
            context.insert("fm_file_ext", e);
        };

        // Read YAML header variable `tag`. If not defined, register nothing.
        if let Some(st) = &fm.sort_tag {
            context.insert("fm_sort_tag", st);
        };
    }

    /// Applies a Tera-template to the notes context in order to generate a
    /// sanitized filename that is in sync with the note's meta data stored in
    /// its front matter.
    pub fn render_filename(&self, template: &str) -> Result<PathBuf> {
        if ARGS.debug {
            eprintln!(
                "*** Debug: Available substitution variables for the filename template:\n{:#?}\n",
                *self.context
            );
            eprintln!(
                "*** Debug: Applying the filename template:\n{}\n\n",
                template
            );
        };

        // render template
        let mut fqfn = self.context.fqpn.to_owned();
        fqfn.push({
            let mut tera = Tera::default();
            tera.extend(&TERA).unwrap();

            tera.render_str(template, &self.context)
                .map(|filename| {
                    if ARGS.debug {
                        eprintln!(
                            "*** Debug: Rendered the filename template:\n{:?}\n\n",
                            filename
                        );
                    };
                    filename
                })
                .with_context(|| format!("Failed to render the template:\n`{}`.", template))?
                .trim()
        });

        Ok(Self::shorten_filename(fqfn))
    }

    /// Shortens the stem of a filename so that
    /// `file_stem.len()+file_extension.len() <= NOTE_FILENAME_LEN_MAX`
    fn shorten_filename(mut fqfn: PathBuf) -> PathBuf {
        // Determine length of file-extension.
        let note_extension = fqfn
            .extension()
            .unwrap_or_default()
            .to_str()
            .unwrap_or_default();
        let note_extension_len = note_extension.len();

        // Limit length of file-stem.
        let note_stem = fqfn
            .file_stem()
            .unwrap_or_default()
            .to_str()
            .unwrap_or_default();

        // Limit the size of `fqfn`
        let mut note_stem_short = String::new();
        // `+1` reserves one byte for `.` before the extension.
        for i in (0..NOTE_FILENAME_LEN_MAX - (note_extension_len + 1)).rev() {
            if let Some(s) = note_stem.get(..=i) {
                note_stem_short = s.to_string();
                break;
            }
        }

        // Assemble.
        let mut note_filename = note_stem_short;
        note_filename.push('.');
        note_filename.push_str(note_extension);

        // Replace filename
        fqfn.set_file_name(note_filename);

        fqfn
    }

    /// Helper function deserializing the front-matter of an `.md`-file.
    fn deserialize_note(header: &str) -> Result<FrontMatter> {
        if header.is_empty() {
            return Err(anyhow!("no YAML front matter found"));
        };

        let fm: FrontMatter = serde_yaml::from_str(&header)?;

        // `sort_tag` has additional constrains to check.
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
        if let Some(extension) = &fm.file_ext {
            let mut extension_is_known = false;
            for e in &CFG.note_file_extensions {
                if e == extension {
                    extension_is_known = true;
                    break;
                }
            }
            if !extension_is_known {
                return Err(anyhow!(format!(
                    "`file_ext=\"{}\"`, is not registered as a valid\n\
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
    pub fn write_to_disk(&self, new_fqfn: PathBuf) -> Result<PathBuf, anyhow::Error> {
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
                    return Err(anyhow!(format!(
                        "Can not write new note, file exists:\n\
                         \t{:?}\n{}",
                        new_fqfn, e
                    )));
                } else {
                    return Err(anyhow!(format!(
                        "Can not write file: {:?}\n{}",
                        new_fqfn, e
                    )));
                }
            }
        }

        Ok(new_fqfn)
    }
}

#[cfg(test)]
mod tests {
    use super::FrontMatter;
    use super::Note;

    #[test]
    fn test_shorten_filename() {
        use std::ffi::OsString;
        use std::path::PathBuf;

        // Test short filename.
        let input = PathBuf::from("long directory name/abc.ext");
        let output = Note::shorten_filename(input);
        assert_eq!(OsString::from("long directory name/abc.ext"), output);

        // Test long filename.
        let input = PathBuf::from("long directory name/long filename.ext");
        let output = Note::shorten_filename(input);
        assert_eq!(OsString::from("long directory name/long f.ext"), output);
    }

    #[test]
    fn test_deserialize() {
        let input = "# document start
        title: The book
        subtitle: you always wanted
        author: It's me
        date: 2020-04-21
        lang: en
        revision: 1.0
        ";

        let expected_front_matter = FrontMatter {
            title: "The book".to_string(),
            subtitle: Some("you always wanted".to_string()),
            author: Some("It's me".to_string()),
            date: Some("2020-04-21".to_string()),
            lang: Some("en".to_string()),
            revision: Some("1.0".to_string()),
            sort_tag: None,
            file_ext: None,
        };

        assert_eq!(
            expected_front_matter,
            Note::deserialize_note(&input).unwrap()
        );

        // Front matter can also end with '---'

        let input = "# document start
        title: \"The book\"
        subtitle: you always wanted
        author: It's me";

        let expected_front_matter = FrontMatter {
            title: "The book".to_string(),
            subtitle: Some("you always wanted".to_string()),
            author: Some("It's me".to_string()),
            date: None,
            lang: None,
            revision: None,
            sort_tag: None,
            file_ext: None,
        };

        assert_eq!(
            expected_front_matter,
            Note::deserialize_note(&input).unwrap()
        );

        // Front matter can optionally have a tag and an extension

        let input = "# document start
        title: \"The book\"
        subtitle: you always wanted
        author: It's me
        sort_tag: 20200420-21_22
        file_ext: md";

        let expected_front_matter = FrontMatter {
            title: "The book".to_string(),
            subtitle: Some("you always wanted".to_string()),
            sort_tag: Some("20200420-21_22".to_string()),
            file_ext: Some("md".to_string()),
            author: Some("It's me".to_string()),
            date: None,
            lang: None,
            revision: None,
        };

        assert_eq!(
            expected_front_matter,
            Note::deserialize_note(&input).unwrap()
        );

        // Is empty.

        let input = "";

        assert!(Note::deserialize_note(&input).is_err());

        // Missing title

        let input = "# document start
        titlxxx: The book
        subtitle: you always wanted
        author: It's me";

        assert!(Note::deserialize_note(&input).is_err());

        // Missing subtitle

        let input = "# document start
        title: The book
        subtitlxxx: you always wanted
        author: It's me";

        let expected_front_matter = FrontMatter {
            title: "The book".to_string(),
            subtitle: None,
            author: Some("It's me".to_string()),
            date: None,
            lang: None,
            revision: None,
            sort_tag: None,
            file_ext: None,
        };

        assert_eq!(
            expected_front_matter,
            Note::deserialize_note(&input).unwrap()
        );

        // forbidden character `x` in `tag`.

        let input = "# document start
        title: The book
        subtitle: you always wanted
        author: It's me
        sort_tag:    123x4";

        assert!(Note::deserialize_note(&input).is_err());
    }
}
