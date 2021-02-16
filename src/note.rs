//! Creates a memory representations of the note by inserting `tp-note`'s
//! environment data in some templates. If the note exists on disk already,
//! the memory representation is established be reading the note-file with
//! its front matter.

use crate::config::ARGS;
use crate::config::CFG;
use crate::config::CLIPBOARD;
use crate::config::STDIN;
use crate::content::Content;
use crate::filename;
use crate::filename::MarkupLanguage;
use crate::filter::ContextWrapper;
use crate::filter::TERA;
use anyhow::{anyhow, Context, Result};
use parse_hyperlinks::renderer::text_links2html;
#[cfg(feature = "viewer")]
use pulldown_cmark::{html, Options, Parser};
#[cfg(feature = "viewer")]
use rst_parser::parse;
#[cfg(feature = "viewer")]
use rst_renderer::render_html;
use std::default::Default;
use std::env;
use std::fs::OpenOptions;
use std::io;
use std::io::Write;
use std::matches;
use std::path::{Path, PathBuf};
use std::pin::Pin;
use std::str;
use tera::Tera;
#[derive(Debug, PartialEq)]
/// Represents a note.
pub struct Note<'a> {
    // Reserved for future use:
    //     /// The front matter of the note.
    //     front_matter: FrontMatter,
    /// Captured environment of `tp-note` that
    /// is used to fill in templates.
    pub context: ContextWrapper,
    /// The full text content of the note, including
    /// its front matter.
    pub content: Pin<Box<Content<'a>>>,
}

#[derive(Debug, PartialEq)]
/// Represents the front matter of the note.
struct FrontMatter {
    map: tera::Map<String, tera::Value>,
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

        // Register the raw serialized header text.
        (*context).insert("fm_all_yaml", &content.header);

        // Deserialize the note read from disk.
        let fm = Note::deserialize_header(content.header)?;

        if !&CFG.tmpl_compulsory_field_content.is_empty()
            && fm.map.get(&CFG.tmpl_compulsory_field_content).is_none()
        {
            return Err(anyhow!(
                "The document is missing a `{}:` field in its front matter:\n\
                 \n\
                 \t~~~~~~~~~~~~~~\n\
                 \t---\n\
                 \t{}: \"My note\"\n\
                 \t---\n\
                 \tsome text\n\
                 \t~~~~~~~~~~~~~~",
                CFG.tmpl_compulsory_field_content,
                CFG.tmpl_compulsory_field_content
            ));
        }

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
                tera.extend(&TERA)?;

                tera.render_str(template, &context)?
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
        (*context).insert("file", &file);

        // `fqpn` is a directory as fully qualified path, ending
        // by a separator.
        let fqpn = if path.is_dir() {
            path
        } else {
            path.parent().unwrap_or_else(|| Path::new("./"))
        };
        (*context).insert("path", &fqpn.to_str().unwrap_or_default());

        // Register input from clipboard.
        (*context).insert("clipboard_header", CLIPBOARD.header);
        (*context).insert("clipboard", CLIPBOARD.body);

        // Register input from stdin.
        (*context).insert("stdin_header", STDIN.header);
        (*context).insert("stdin", STDIN.body);

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
            return Err(anyhow!(
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
            ));
        };

        // Register clipboard front matter.
        if let Some(fm) = clipboard_fm {
            Self::register_front_matter(&mut context, &fm);
        }

        // Register stdin front matter.
        // The variables registered here can be overwrite the ones from the clipboard.
        if let Some(fm) = stdin_fm {
            Self::register_front_matter(&mut context, &fm);
        }

        // Default extension for new notes as defined in the configuration file.
        (*context).insert("extension_default", CFG.extension_default.as_str());

        // search for UNIX, Windows and MacOS user-names
        let author = env::var("LOGNAME").unwrap_or_else(|_| {
            env::var("USERNAME").unwrap_or_else(|_| env::var("USER").unwrap_or_default())
        });
        (*context).insert("username", &author);

        context.fqpn = fqpn.to_path_buf();

        Ok(context)
    }

    /// Copies the YAML front header variable in the context for later use with templates.
    /// We register only flat `tera::Value` types.
    /// If there is a list, concatenate its items with `, ` and register the result
    /// as a flat string.
    fn register_front_matter(context: &mut ContextWrapper, fm: &FrontMatter) {
        let mut tera_map = tera::Map::new();

        for (name, value) in &fm.map {
            // Flatten all types.
            let val = match value {
                tera::Value::String(_) => value.to_owned(),
                tera::Value::Number(_) => value.to_owned(),
                tera::Value::Bool(_) => value.to_owned(),
                _ => tera::Value::String(value.to_string()),
            };

            // First we register a copy with the original variable name.
            tera_map.insert(name.to_string(), val.to_owned());

            // Here we register `fm_<var_name>`.
            let mut var_name = "fm_".to_string();
            var_name.push_str(name);
            (*context).insert(&var_name, &val);
        }
        // Register the collection as `Object(Map<String, Value>)`.
        (*context).insert("fm_all", &tera_map);
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
            tera.extend(&TERA)?;

            tera.render_str(template, &self.context)
                .map(|filename| {
                    if ARGS.debug {
                        eprintln!(
                            "*** Debug: Rendered the filename template:\n{:?}\n\n",
                            filename
                        );
                    };
                    filename
                })?
                .trim()
        });

        Ok(filename::shorten_filename(fqfn))
    }

    /// Helper function deserializing the front-matter of an `.md`-file.
    fn deserialize_header(header: &str) -> Result<FrontMatter> {
        if header.is_empty() {
            return Err(anyhow!(
                "The document (or template) has no front matter section.\n\
                 Is one `---` missing?\n\n\
                 \t~~~~~~~~~~~~~~\n\
                 \t---\n\
                 \t{}: \"My note\"\n\
                 \t---\n\
                 \tsome text\n\
                 \t~~~~~~~~~~~~~~",
                CFG.tmpl_compulsory_field_content
            ));
        };

        let map: tera::Map<String, tera::Value> = serde_yaml::from_str(&header)?;
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
                    return Err(anyhow!(
                        "The `sort_tag` header variable contains forbidden character(s): sort_tag = \"{}\". \
                        Only numbers, `-` and `_` are allowed here.",
                        sort_tag
                    ));
                }
            };
        };

        // `extension` has also additional constrains to check.
        // Is `extension` listed in `CFG.note_file_extension`?
        if let Some(tera::Value::String(extension)) = &fm.map.get("file_ext") {
            let extension_is_unknown =
                matches!(MarkupLanguage::new(extension), MarkupLanguage::None);
            if extension_is_unknown {
                return Err(anyhow!(
                    "`file_ext=\"{}\"`, is not registered as a valid\n\
                        Tp-Note-file in the `note_file_extensions_*` variables\n\
                        in your configuration file:\n\
                        \t{:?}\n\
                        \t{:?}\n\
                        \t{:?}\n\
                        \t{:?}\n\
                        \t{:?}\n\
                        \n\
                        Choose one of the above list or add more extensions to the\n\
                        `note_file_extensions_*` variables in your configuration file.",
                    extension,
                    &CFG.note_file_extensions_md,
                    &CFG.note_file_extensions_rst,
                    &CFG.note_file_extensions_html,
                    &CFG.note_file_extensions_txt,
                    &CFG.note_file_extensions_no_viewer,
                ));
            }
        };

        Ok(fm)
    }

    /// Renders `self` into HTML and saves the result in `export_dir`. If
    /// `export_dir` is the empty string, the directory of `note_path` is
    /// used. `-` dumps the rendition to STDOUT.
    pub fn render_and_write_content(
        &mut self,
        note_path: &Path,
        export_dir: &Path,
    ) -> Result<(), anyhow::Error> {
        // Determine filename of html-file.
        let mut html_path = PathBuf::new();
        if export_dir
            .as_os_str()
            .to_str()
            .unwrap_or_default()
            .is_empty()
        {
            html_path = note_path.parent().unwrap_or(Path::new("")).to_path_buf();
            let mut html_filename = note_path
                .file_name()
                .unwrap_or_default()
                .to_str()
                .unwrap_or_default()
                .to_string();
            html_filename.push_str(".html");
            html_path.push(PathBuf::from(html_filename.as_str()));
        } else if export_dir.as_os_str().to_str().unwrap_or_default() != "-" {
            html_path = export_dir.to_owned();
            let mut html_filename = note_path
                .file_name()
                .unwrap_or_default()
                .to_str()
                .unwrap_or_default()
                .to_string();
            html_filename.push_str(".html");
            html_path.push(PathBuf::from(html_filename.as_str()));
        } else {
            // `export_dir` points to `-` and `html_path` is empty.
        }

        if ARGS.debug {
            if html_path
                .as_os_str()
                .to_str()
                .unwrap_or_default()
                .is_empty()
            {
                eprintln!("*** Debug: rendering HTML to STDOUT (`{:?}`)", export_dir);
            } else {
                eprintln!("*** Debug: rendering HTML into: {:?}", html_path);
            }
        };

        // The file extension identifies the markup language.
        let note_path_ext = note_path
            .extension()
            .unwrap_or_default()
            .to_str()
            .unwrap_or_default();

        // Check where to dump output.
        if html_path
            .as_os_str()
            .to_str()
            .unwrap_or_default()
            .is_empty()
        {
            let stdout = io::stdout();
            let mut handle = stdout.lock();

            // Write HTML rendition.
            handle.write_all(
                self.render_content(&note_path_ext, &CFG.exporter_rendition_tmpl, "")?
                    .as_bytes(),
            )?;
        } else {
            let mut handle = OpenOptions::new()
                .write(true)
                .create(true)
                .open(&html_path)?;
            // Write HTML rendition.
            handle.write_all(
                self.render_content(&note_path_ext, &CFG.exporter_rendition_tmpl, "")?
                    .as_bytes(),
            )?;
        };
        Ok(())
    }

    #[inline]
    /// First, determines the markup language from the file extension or
    /// the `fm_file_ext` YAML variable, if present.
    /// Then calls the appropriate markup renderer.
    /// Finally the result is rendered with the `VIEWER_RENDITION_TMPL`
    /// template.
    pub fn render_content(
        &mut self,
        // We need the file extension to determine the
        // markup language.
        file_ext: &str,
        // HTML template for this rendition.
        tmpl: &str,
        // If not empty, Java-Script code to inject in output.
        java_script: &str,
    ) -> Result<String, anyhow::Error> {
        // Deserialize.

        // Render Body.
        let input = self.content.body;

        // What Markup language is used?
        let ext = match self.context.get("fm_file_ext") {
            Some(tera::Value::String(file_ext)) => Some(file_ext.as_str()),
            _ => None,
        };

        // Render the markup language.
        let html_output = match MarkupLanguage::from(ext, &file_ext) {
            #[cfg(feature = "viewer")]
            MarkupLanguage::Markdown => Self::render_md_content(input),
            #[cfg(feature = "viewer")]
            MarkupLanguage::RestructuredText => Self::render_rst_content(input)?,
            MarkupLanguage::Html => input.to_string(),
            _ => Self::render_txt_content(input),
        };

        // Register rendered body.
        self.context.insert("noteBody", &html_output);

        // Java Script
        self.context.insert("noteJS", java_script);

        let mut tera = Tera::default();
        tera.extend(&TERA)?;
        let html = tera.render_str(tmpl, &self.context)?;
        Ok(html)
    }

    #[inline]
    #[cfg(feature = "viewer")]
    /// Markdown renderer.
    fn render_md_content(markdown_input: &str) -> String {
        // Set up options and parser. Besides the CommonMark standard
        // we enable some useful extras.
        let options = Options::all();
        let parser = Parser::new_ext(markdown_input, options);

        // Write to String buffer.
        let mut html_output: String = String::with_capacity(markdown_input.len() * 3 / 2);
        html::push_html(&mut html_output, parser);
        html_output
    }

    #[inline]
    #[cfg(feature = "viewer")]
    /// RestructuredText renderer.
    fn render_rst_content(rest_input: &str) -> Result<String, anyhow::Error> {
        // Note, that the current rst renderer requires files to end with a new line.
        // <https://github.com/flying-sheep/rust-rst/issues/30>
        let mut rest_input = rest_input.trim_start();
        // The rst parser accepts only exactly one newline at the end.
        while rest_input.ends_with("\n\n") {
            rest_input = &rest_input[..rest_input.len() - 1];
        }
        let document = parse(rest_input.trim_start()).map_err(|e| anyhow!(e))?;
        // Write to String buffer.
        let mut html_output: Vec<u8> = Vec::with_capacity(rest_input.len() * 3 / 2);
        let _ = render_html(&document, &mut html_output, false);
        Ok(str::from_utf8(&html_output)?.to_string())
    }

    #[inline]
    /// Renderer for markup languages other than the above.
    fn render_txt_content(other_input: &str) -> String {
        text_links2html(other_input)
    }
}

#[cfg(test)]
mod tests {
    use super::ContextWrapper;
    use super::FrontMatter;
    use super::Note;
    use serde_json::json;
    use tera::Value;

    #[test]
    fn test_deserialize() {
        let input = "# document start
        title:     The book
        subtitle:  you always wanted
        author:    It's me
        date:      2020-04-21
        lang:      en
        revision:  '1.0'
        sort_tag:  20200420-21_22
        file_ext:  md
        height:    1.23
        count:     2
        neg:       -1
        flag:      true
        numbers:
          - 1
          - 3
          - 5
        ";

        let mut expected = tera::Map::new();
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
        expected.insert("height".to_string(), json!(1.23)); // Number()
        expected.insert("count".to_string(), json!(2)); // Number()
        expected.insert("neg".to_string(), json!(-1)); // Number()
        expected.insert("flag".to_string(), json!(true)); // Bool()
        expected.insert("numbers".to_string(), json!([1, 3, 5])); // Array()

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

    #[test]
    fn test_register_front_matter() {
        let mut tmp = tera::Map::new();
        tmp.insert("file_ext".to_string(), Value::String("md".to_string())); // String
        tmp.insert("height".to_string(), json!(1.23)); // Number()
        tmp.insert("count".to_string(), json!(2)); // Number()
        tmp.insert("neg".to_string(), json!(-1)); // Number()
        tmp.insert("flag".to_string(), json!(true)); // Bool()
        tmp.insert("numbers".to_string(), json!([1, 3, 5])); // Array([Numbers()..])!
        let mut tmp2 = tmp.clone();

        let mut input1 = ContextWrapper::new();
        let input2 = FrontMatter { map: tmp };

        let mut expected = ContextWrapper::new();
        (*expected).insert("fm_file_ext".to_string(), &json!("md")); // String
        (*expected).insert("fm_height".to_string(), &json!(1.23)); // Number()
        (*expected).insert("fm_count".to_string(), &json!(2)); // Number()
        (*expected).insert("fm_neg".to_string(), &json!(-1)); // Number()
        (*expected).insert("fm_flag".to_string(), &json!(true)); // Bool()
        (*expected).insert("fm_numbers".to_string(), &json!("[1,3,5]")); // String()!
        tmp2.remove("numbers");
        tmp2.insert("numbers".to_string(), json!("[1,3,5]")); // String()!
        (*expected).insert("fm_all".to_string(), &tmp2); // Map()

        Note::register_front_matter(&mut input1, &input2);
        let result = input1;

        assert_eq!(result, expected);
    }
}
