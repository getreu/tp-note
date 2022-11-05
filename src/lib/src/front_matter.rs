//! Creates a memory representation of the note's YAML header.
//! In this documentation, the terms “YAML header”, ”header” and ”front matter"
//! are used as synonyms for the note's meta data block at the beginning
//! of the text file. Technically this is a wrapper around a `tera::Map`.
use crate::config::LIB_CFG;
use crate::config::TMPL_VAR_FM_;
use crate::config::TMPL_VAR_FM_FILE_EXT;
use crate::config::TMPL_VAR_FM_SORT_TAG;
use crate::content::Content;
use crate::error::NoteError;
use crate::error::FRONT_MATTER_ERROR_MAX_LINES;
use crate::filename::MarkupLanguage;
use std::matches;
use std::ops::Deref;
use std::ops::DerefMut;
use std::str;

#[derive(Debug, Eq, PartialEq)]
/// Represents the front matter of the note. This is a newtype
/// for `tera::Map<String, tera::Value>`.
pub struct FrontMatter(pub tera::Map<String, tera::Value>);

impl FrontMatter {
    /// Checks if the front matter contains a field variable
    /// with the name defined in the configuration file:
    /// as: "compulsory_header_field".
    pub fn assert_compulsory_field(&self) -> Result<(), NoteError> {
        let lib_cfg = LIB_CFG.read().unwrap();

        if !(*lib_cfg).tmpl.compulsory_header_field.is_empty() {
            if let Some(tera::Value::String(header_field)) =
                self.get(&(*lib_cfg).tmpl.compulsory_header_field)
            {
                if header_field.is_empty() {
                    return Err(NoteError::CompulsoryFrontMatterFieldIsEmpty {
                        field_name: (*lib_cfg).tmpl.compulsory_header_field.to_owned(),
                    });
                };
            } else {
                return Err(NoteError::MissingFrontMatterField {
                    field_name: (*lib_cfg).tmpl.compulsory_header_field.to_owned(),
                });
            }
        }
        Ok(())
    }

    /// Are any variables registerd?
    pub fn assert_not_empty(&self) -> Result<(), NoteError> {
        if self.is_empty() {
            let lib_cfg = LIB_CFG.read().unwrap();
            Err(NoteError::MissingFrontMatter {
                compulsory_field: (*lib_cfg).tmpl.compulsory_header_field.to_owned(),
            })
        } else {
            Ok(())
        }
    }

    /// Helper function deserialising the front-matter of the note file.
    ///
    /// ```rust
    /// use tpnote_lib::content::Content;
    /// use tpnote_lib::content::ContentString;
    /// use tpnote_lib::front_matter::FrontMatter;
    /// use serde_json::json;

    /// // Create existing note.
    /// let raw = "\u{feff}---\ntitle: \"My day\"\nsubtitle: \"Note\"\n---\nBody";
    /// let content = ContentString::from(raw.to_string());
    /// assert!(!content.is_empty());
    /// assert!(!content.borrow_dependent().header.is_empty());
    ///
    /// let front_matter = FrontMatter::try_from_content(&content).unwrap();
    /// assert_eq!(front_matter.get("title"), Some(&json!("My day")));
    /// assert_eq!(front_matter.get("subtitle"), Some(&json!("Note")));
    /// ```
    pub fn try_from_content(content: &impl Content) -> Result<FrontMatter, NoteError> {
        let header = content.header();
        Self::try_from(header)
    }
}

impl TryFrom<&str> for FrontMatter {
    type Error = NoteError;
    /// Helper function deserialising the front-matter of the note file.
    fn try_from(header: &str) -> Result<FrontMatter, NoteError> {
        let lib_cfg = LIB_CFG.read().unwrap();

        let map: tera::Map<String, tera::Value> =
            serde_yaml::from_str(header).map_err(|e| NoteError::InvalidFrontMatterYaml {
                front_matter: header
                    .lines()
                    .enumerate()
                    .map(|(n, s)| format!("{:03}: {}\n", n + 1, s))
                    .take(FRONT_MATTER_ERROR_MAX_LINES)
                    .collect::<String>(),
                source_error: e,
            })?;
        let fm = FrontMatter(map);

        // `sort_tag` has additional constrains to check.
        if let Some(tera::Value::String(sort_tag)) =
            &fm.get(TMPL_VAR_FM_SORT_TAG.trim_start_matches(TMPL_VAR_FM_))
        {
            if !sort_tag.is_empty() {
                // Check for forbidden characters.
                if !sort_tag
                    .trim_start_matches(
                        &lib_cfg
                            .filename
                            .sort_tag_chars
                            .chars()
                            .collect::<Vec<char>>()[..],
                    )
                    .is_empty()
                {
                    return Err(NoteError::SortTagVarInvalidChar {
                        sort_tag: sort_tag.to_owned(),
                        sort_tag_chars: lib_cfg
                            .filename
                            .sort_tag_chars
                            .escape_default()
                            .to_string(),
                    });
                }
            };
        };

        // `extension` has also additional constrains to check.
        // Is `extension` listed in `CFG.filename.extensions_*`?
        if let Some(tera::Value::String(file_ext)) =
            &fm.get(TMPL_VAR_FM_FILE_EXT.trim_start_matches(TMPL_VAR_FM_))
        {
            let extension_is_unknown =
                matches!(MarkupLanguage::from(&**file_ext), MarkupLanguage::None);
            if extension_is_unknown {
                return Err(NoteError::FileExtNotRegistered {
                    extension: file_ext.to_owned(),
                    md_ext: lib_cfg.filename.extensions_md.to_owned(),
                    rst_ext: lib_cfg.filename.extensions_rst.to_owned(),
                    html_ext: lib_cfg.filename.extensions_html.to_owned(),
                    txt_ext: lib_cfg.filename.extensions_txt.to_owned(),
                    no_viewer_ext: lib_cfg.filename.extensions_no_viewer.to_owned(),
                });
            }
        }
        Ok(fm)
    }
}

/// Auto-dereference for convenient access to `tera::Map`.
impl Deref for FrontMatter {
    type Target = tera::Map<String, tera::Value>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

/// Auto-dereference for convenient access to `tera::Map`.
impl DerefMut for FrontMatter {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

#[cfg(test)]
mod tests {
    use super::FrontMatter;
    use crate::context::Context;
    use serde_json::json;
    use std::path::Path;
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

        let expected_front_matter = FrontMatter(expected);

        assert_eq!(expected_front_matter, FrontMatter::try_from(input).unwrap());

        //
        // Is empty.
        let input = "";

        assert!(FrontMatter::try_from(input).is_ok());

        //
        // forbidden character `x` in `tag`.
        let input = "# document start
        title: The book
        subtitle: you always wanted
        author: It's me
        sort_tag:    123x4";

        assert!(FrontMatter::try_from(input).is_err());

        //
        // Not registered file extension.
        let input = "# document start
        title: The book
        subtitle: you always wanted
        author: It's me
        sort_tag:    123x4
        file_ext:    xyz";

        assert!(FrontMatter::try_from(input).is_err());
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

        let mut input1 = Context::from(Path::new("a/b/test.md"));
        let input2 = FrontMatter(tmp);

        let mut expected = Context::from(Path::new("a/b/test.md"));
        (*expected).insert("fm_file_ext".to_string(), &json!("md")); // String
        (*expected).insert("fm_height".to_string(), &json!(1.23)); // Number()
        (*expected).insert("fm_count".to_string(), &json!(2)); // Number()
        (*expected).insert("fm_neg".to_string(), &json!(-1)); // Number()
        (*expected).insert("fm_flag".to_string(), &json!(true)); // Bool()
        (*expected).insert("fm_numbers".to_string(), &json!("[1,3,5]")); // String()!
        tmp2.remove("numbers");
        tmp2.insert("numbers".to_string(), json!("[1,3,5]")); // String()!
        (*expected).insert("fm_all".to_string(), &tmp2); // Map()

        input1.insert_front_matter(&input2);
        let result = input1;

        assert_eq!(result, expected);
    }
}
