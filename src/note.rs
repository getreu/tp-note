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
use crate::config::STDIN;
use crate::content::Content;
use crate::filename;
use crate::filter::ContextWrapper;
use crate::filter::TERA;
use anyhow::{anyhow, Context, Result};
use std::collections::BTreeMap;
use std::default::Default;
use std::env;
use std::path::{Path, PathBuf};
use std::pin::Pin;
use tera::Tera;

#[derive(Debug, PartialEq)]
/// Represents a note.
pub struct Note<'a> {
    // Reserved for future use:
    //     /// The front matter of the note.
    //     front_matter: FrontMatter,
    /// Captured environment of `tp-note` that
    /// is used to fill in templates.
    context: ContextWrapper,
    /// The full text content of the note, including
    /// its front matter.
    pub content: Pin<Box<Content<'a>>>,
}

#[derive(Debug, PartialEq, Default)]
/// Represents the front matter of the note.
struct FrontMatter {
    map: BTreeMap<String, tera::Value>,
}

use std::fs;
impl Note<'_> {
    /// Constructor that creates a memory representation of an existing note on
    /// disk.
    pub fn from_existing_note(path: &Path) -> Result<Self> {
        let content = Content::new(
            fs::read_to_string(path)
                .with_context(|| format!("Failed to read `{}`.", path.display()))?,
            true,
        );

        let mut context = Self::capture_environment(&path)?;

        // deserialize the note read from disk
        let fm = Note::deserialize_header(content.header)?;

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
        let content = Content::new(
            {
                let mut tera = Tera::default();
                tera.extend(&TERA).unwrap();

                tera.render_str(template, &context)
                    .with_context(|| format!("Failed to render the template:\n`{}`.", template))?
            },
            false,
        );

        if ARGS.debug {
            eprintln!(
                "*** Debug: Available substitution variables for content template:\n{:#?}\n",
                *context
            );
            eprintln!("*** Debug: Applying content template:\n{}\n", template);
            eprintln!(
                "*** Debug: Rendered content template:\n---\n{}\n---\n{}\n\n",
                content.header,
                content.body.trim()
            );
        };

        if content.header.is_empty() {
            return Err(anyhow!(
                "The rendered document structure is not conform\n\
                 with the following convention:\n\
                 \t~~~~~~~~~~~~~~\n\
                 \t---\n\
                 \t<YAML header>\n\
                 \t---\n\
                 \t<note body>\n\
                 \t~~~~~~~~~~~~~~\n\
                 Correct the template in the configuration file and\n\
                 restart Tp-Note with `tp-note --debug`.",
            ));
        };

        // deserialize the rendered template
        let fm = Note::deserialize_header(content.header)?;

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
        context.insert("clipboard_header", CLIPBOARD.header);
        context.insert("clipboard", CLIPBOARD.body);

        // Register input from stdin.
        context.insert("stdin_header", STDIN.header);
        context.insert("stdin", STDIN.body);

        // Can we find a front matter in the input stream? If yes, the
        // unmodified input stream is our new note content.
        let stdin_fm = Self::deserialize_header(STDIN.header).ok();
        if ARGS.debug && stdin_fm.is_some() {
            eprintln!(
                "*** Debug: YAML front matter in the input stream stdin found:\n{:#?}",
                stdin_fm
            );
        };

        // Can we find a front matter in the clipboard? If yes, the unmodified
        // clipboard data is our new note content.
        let clipboard_fm = Self::deserialize_header(CLIPBOARD.header).ok();
        if ARGS.debug && clipboard_fm.is_some() {
            eprintln!(
                "*** Debug: YAML front matter in the clipboard found:\n{:#?}",
                clipboard_fm
            );
        };

        if (!CLIPBOARD.header.is_empty() && clipboard_fm.is_none())
            || (!STDIN.header.is_empty() && stdin_fm.is_none())
        {
            return Err(anyhow!(format!(
                "invalid field(s) in the clipboard's YAML\n\
                     header or in the `stdin` input stream found.
                     {}{}{}{}{}{}",
                if !CLIPBOARD.header.is_empty() {
                    "\n*   Clipboard header:\n---\n"
                } else {
                    ""
                },
                CLIPBOARD.header,
                if !CLIPBOARD.header.is_empty() {
                    "\n---"
                } else {
                    ""
                },
                if !STDIN.header.is_empty() {
                    "\n*   Input stream header:\n---\n"
                } else {
                    ""
                },
                STDIN.header,
                if !STDIN.header.is_empty() {
                    "\n---"
                } else {
                    ""
                },
            )));
        };

        // Register clipboard front matter.
        if let Some(fm) = clipboard_fm {
            // Register YAML header variable `title`. We know, it exists.
            Self::register_front_matter(&mut context, &fm);
        }

        // Register stdin front matter.
        // Variables can be overwrittern by clipboard frontmatter.
        if let Some(fm) = stdin_fm {
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
    /// We register only `tera::Value` types that can be converted to a String.
    /// If there is a list, concatente its items with `, ` and register the result
    /// as a flat string.
    fn register_front_matter(context: &mut ContextWrapper, fm: &FrontMatter) {
        let mut tera_map = tera::Map::new();
        for (name, val) in &fm.map {
            let val = match val {
                tera::Value::String(val) => val.to_string(),
                tera::Value::Number(n) => n.to_string(),
                tera::Value::Bool(b) => b.to_string(),
                tera::Value::Array(a) => {
                    let mut val = String::new();
                    for v in a {
                        let s = match v {
                            tera::Value::String(v) => v.to_string(),
                            tera::Value::Number(n) => n.to_string(),
                            tera::Value::Bool(b) => b.to_string(),
                            _ => continue,
                        };
                        val.push_str(&s);
                        val.push_str(", ");
                    }
                    val.trim_end_matches(", ").to_string()
                }
                _ => continue,
            };

            // We keep a copy for the `fm_all` variable.
            tera_map.insert(name.to_string(), tera::Value::String(val.to_string()));

            // Here we register `fm_<var_name>`.
            let mut var_name = "fm_".to_string();
            var_name.push_str(name.as_str());
            context.insert(&var_name, &*val);
        }
        // Register the collection as `Object(Map<String, Value>)`.
        context.insert_map("fm_all", tera_map);
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

        Ok(filename::shorten_filename(fqfn))
    }

    /// Helper function deserializing the front-matter of an `.md`-file.
    fn deserialize_header(header: &str) -> Result<FrontMatter> {
        if header.is_empty() {
            return Err(anyhow!("no YAML front matter found"));
        };

        let map: BTreeMap<String, tera::Value> = serde_yaml::from_str(&header)?;
        let fm = FrontMatter { map };

        // `sort_tag` has additional constrains to check.

        if let Some(tera::Value::String(sort_tag)) = &fm.map.get("sort_tag") {
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
        if let Some(tera::Value::String(extension)) = &fm.map.get("file_ext") {
            let mut extension_is_known = false;
            for e in &CFG.note_file_extensions {
                if *e == *extension {
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
}

#[cfg(test)]
mod tests {
    use super::FrontMatter;
    use super::Note;
    use std::collections::BTreeMap;
    use tera::Value;

    #[test]
    fn test_deserialize() {
        let input = "# document start
        title: The book
        subtitle: you always wanted
        author: It's me
        date: 2020-04-21
        lang: en
        revision: '1.0'
        sort_tag: 20200420-21_22
        file_ext: md
        ";

        let mut expected = BTreeMap::new();
        expected.insert("title".to_string(), Value::String("The book".to_string()));
        expected.insert(
            "subtitle".to_string(),
            Value::String("you always wanted".to_string()),
        );
        expected.insert("author".to_string(), Value::String("It\'s me".to_string()));
        expected.insert("date".to_string(), Value::String("2020-04-21".to_string()));
        expected.insert("lang".to_string(), Value::String("en".to_string()));
        expected.insert("revision".to_string(), Value::String("1.0".to_string()));
        expected.insert(
            "sort_tag".to_string(),
            Value::String("20200420-21_22".to_string()),
        );
        expected.insert("file_ext".to_string(), Value::String("md".to_string()));

        let expected_front_matter = FrontMatter { map: expected };

        assert_eq!(
            expected_front_matter,
            Note::deserialize_header(&input).unwrap()
        );

        //
        // Is empty.

        let input = "";

        assert!(Note::deserialize_header(&input).is_err());

        //
        // forbidden character `x` in `tag`.

        let input = "# document start
        title: The book
        subtitle: you always wanted
        author: It's me
        sort_tag:    123x4";

        assert!(Note::deserialize_header(&input).is_err());

        //
        // Not registered file extension.

        let input = "# document start
        title: The book
        subtitle: you always wanted
        author: It's me
        sort_tag:    123x4
        file_ext:    xyz";

        assert!(Note::deserialize_header(&input).is_err());
    }
}
