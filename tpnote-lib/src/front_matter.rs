//! Creates a memory representation of the note's YAML header.
//! In this documentation, the terms “YAML header”, ”header” and ”front matter"
//! are used as synonyms for the note's meta data block at the beginning
//! of the text file. Technically this is a wrapper around a `tera::Map`.
use crate::config::Assertion;
use crate::config::LIB_CFG;
use crate::error::LibCfgError;
use crate::error::NoteError;
use crate::error::FRONT_MATTER_ERROR_MAX_LINES;
use crate::filename::Extension;
use crate::filename::NotePath;
use crate::filename::NotePathStr;
use crate::filter::name;
use crate::settings::SETTINGS;
use std::matches;
use std::ops::Deref;
use std::ops::DerefMut;
use std::path::Path;
use std::str;
use tera::Value;

#[derive(Debug, Eq, PartialEq)]
/// Represents the front matter of the note. This is a newtype
/// for `tera::Map<String, tera::Value>`.
pub struct FrontMatter(pub tera::Map<String, tera::Value>);

/// Helper function asserting that all the leaves of `val` have a certain type.
/// The first parameter is the type to check recursively.
/// The second is a closure that evaluates to true or false.
fn all_leaves(val: &Value, f: &dyn Fn(&Value) -> bool) -> bool {
    match &val {
        Value::Array(a) => {
            for i in a.iter() {
                if !all_leaves(i, &f) {
                    return false;
                }
            }
        }
        Value::Object(map) => {
            for (_, v) in map {
                if !all_leaves(v, &f) {
                    return false;
                }
            }
        }

        _ => {
            return f(val);
        }
    }
    true
}

impl FrontMatter {
    /// Checks if the front matter variables satisfy preconditions.
    /// The path is the path to the current document.
    #[inline]
    pub fn assert_precoditions(&self, docpath: &Path) -> Result<(), NoteError> {
        let lib_cfg = &LIB_CFG.read_recursive();
        let scheme = &lib_cfg.scheme[SETTINGS.read_recursive().current_scheme];
        for (key, conditions) in scheme.tmpl.fm_var.assertions.iter() {
            let localized_key = name(scheme, key);
            if let Some(value) = self.get(localized_key) {
                for cond in conditions {
                    match cond {
                        Assertion::IsDefined => {}

                        Assertion::IsString => {
                            if !all_leaves(value, &|v| matches!(v, Value::String(..))) {
                                return Err(NoteError::FrontMatterFieldIsNotString {
                                    field_name: localized_key.to_owned(),
                                });
                            }
                        }

                        Assertion::IsNotEmptyString => {
                            if !all_leaves(value, &|v| {
                                matches!(v, Value::String(..)) && v.as_str() != Some("")
                            }) {
                                return Err(NoteError::FrontMatterFieldIsEmptyString {
                                    field_name: localized_key.to_owned(),
                                });
                            }
                        }

                        Assertion::IsNumber => {
                            if !all_leaves(value, &|v| matches!(v, Value::Number(..))) {
                                return Err(NoteError::FrontMatterFieldIsNotNumber {
                                    field_name: localized_key.to_owned(),
                                });
                            }
                        }

                        Assertion::IsBool => {
                            if !all_leaves(value, &|v| matches!(v, Value::Bool(..))) {
                                return Err(NoteError::FrontMatterFieldIsNotBool {
                                    field_name: localized_key.to_owned(),
                                });
                            }
                        }

                        Assertion::IsNotCompound => {
                            if matches!(value, Value::Array(..))
                                || matches!(value, Value::Object(..))
                            {
                                return Err(NoteError::FrontMatterFieldIsCompound {
                                    field_name: localized_key.to_owned(),
                                });
                            }
                        }

                        Assertion::IsValidSortTag => {
                            let fm_sort_tag = value.as_str().unwrap_or_default();
                            if !fm_sort_tag.is_empty() {
                                // Check for forbidden characters.
                                let (_, rest, is_sequential) = fm_sort_tag.split_sort_tag(true);
                                if !rest.is_empty() {
                                    return Err(NoteError::FrontMatterFieldIsInvalidSortTag {
                                        sort_tag: fm_sort_tag.to_owned(),
                                        sort_tag_extra_chars: scheme
                                            .filename
                                            .sort_tag
                                            .extra_chars
                                            .escape_default()
                                            .to_string(),
                                        filename_sort_tag_letters_in_succession_max: scheme
                                            .filename
                                            .sort_tag
                                            .letters_in_succession_max,
                                    });
                                }

                                // Check for duplicate sequential sort-tags.
                                if !is_sequential {
                                    // No further checks.
                                    return Ok(());
                                }
                                let docpath = docpath.to_str().unwrap_or_default();

                                let (dirpath, filename) =
                                    docpath.rsplit_once(['/', '\\']).unwrap_or(("", docpath));
                                let sort_tag = filename.split_sort_tag(false).0;
                                // No further check if filename(path) has no sort-tag
                                // or if sort-tags are identical.
                                if sort_tag.is_empty() || sort_tag == fm_sort_tag {
                                    return Ok(());
                                }
                                let dirpath = Path::new(dirpath);

                                if let Some(other_file) =
                                    dirpath.has_file_with_sort_tag(fm_sort_tag)
                                {
                                    return Err(NoteError::FrontMatterFieldIsDuplicateSortTag {
                                        sort_tag: fm_sort_tag.to_string(),
                                        existing_file: other_file,
                                    });
                                }
                            }
                        }

                        Assertion::IsTpnoteExtension => {
                            let file_ext = value.as_str().unwrap_or_default();

                            if !file_ext.is_empty() && !(*file_ext).is_tpnote_ext() {
                                return Err(NoteError::FrontMatterFieldIsNotTpnoteExtension {
                                    extension: file_ext.to_string(),
                                    extensions: {
                                        use std::fmt::Write;
                                        let mut errstr = scheme.filename.extensions.iter().fold(
                                            String::new(),
                                            |mut output, (k, _v1, _v2)| {
                                                let _ = write!(output, "{k}, ");
                                                output
                                            },
                                        );
                                        errstr.truncate(errstr.len().saturating_sub(2));
                                        errstr
                                    },
                                });
                            }
                        }

                        Assertion::IsConfiguredScheme => {
                            let fm_scheme = value.as_str().unwrap_or_default();
                            match lib_cfg.scheme_idx(fm_scheme) {
                                Ok(_) => {}
                                Err(LibCfgError::SchemeNotFound {
                                    scheme_name,
                                    schemes,
                                }) => {
                                    return Err(NoteError::SchemeNotFound {
                                        scheme_val: scheme_name,
                                        scheme_key: localized_key.to_string(),
                                        schemes,
                                    })
                                }
                                Err(e) => return Err(e.into()),
                            };
                        }

                        Assertion::NoOperation => {}
                    } //
                }
                //
            } else if conditions.contains(&Assertion::IsDefined) {
                return Err(NoteError::FrontMatterFieldMissing {
                    field_name: localized_key.to_owned(),
                });
            }
        }
        Ok(())
    }
}

impl TryFrom<&str> for FrontMatter {
    type Error = NoteError;
    /// Helper function deserializing the front-matter of the note file.
    /// An empty header leads to an empty `tera::Map`; no error.
    fn try_from(header: &str) -> Result<FrontMatter, NoteError> {
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

        Ok(fm)
    }
}

/// Auto dereferences for convenient access to `tera::Map`.
impl Deref for FrontMatter {
    type Target = tera::Map<String, tera::Value>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

/// Auto dereferences for convenient access to `tera::Map`.
impl DerefMut for FrontMatter {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use crate::config::TMPL_VAR_FM_ALL;
    use crate::error::NoteError;
    use crate::front_matter::FrontMatter;
    use serde_json::json;
    use tera::Value;

    #[test]
    fn test_deserialize() {
        use super::FrontMatter;
        use serde_json::json;
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
    }

    #[test]
    fn test_register_front_matter() {
        use super::FrontMatter;
        use crate::context::Context;
        use serde_json::json;
        use std::path::Path;
        use tera::Value;

        let mut tmp = tera::Map::new();
        tmp.insert("file_ext".to_string(), Value::String("md".to_string())); // String
        tmp.insert("height".to_string(), json!(1.23)); // Number()
        tmp.insert("count".to_string(), json!(2)); // Number()
        tmp.insert("neg".to_string(), json!(-1)); // Number()
        tmp.insert("flag".to_string(), json!(true)); // Bool()
        tmp.insert("numbers".to_string(), json!([1, 3, 5])); // Array([Numbers()..])!
        let mut tmp2 = tera::Map::new();
        tmp2.insert("fm_file_ext".to_string(), Value::String("md".to_string())); // String
        tmp2.insert("fm_height".to_string(), json!(1.23)); // Number()
        tmp2.insert("fm_count".to_string(), json!(2)); // Number()
        tmp2.insert("fm_neg".to_string(), json!(-1)); // Number()
        tmp2.insert("fm_flag".to_string(), json!(true)); // Bool()
        tmp2.insert("fm_numbers".to_string(), json!([1, 3, 5])); // Array([Numbers()..])!

        let mut input1 = Context::from(Path::new("a/b/test.md"));
        let input2 = FrontMatter(tmp);

        let mut expected = Context::from(Path::new("a/b/test.md"));
        tmp2.remove("fm_numbers");
        tmp2.insert("fm_numbers".to_string(), json!([1, 3, 5])); // String()!
        (*expected).insert(TMPL_VAR_FM_ALL.to_string(), &tmp2); // Map()

        input1.insert_front_matter(&input2);
        let result = input1;

        assert_eq!(result, expected);
    }

    #[test]
    fn test_try_from_content() {
        use crate::content::Content;
        use crate::content::ContentString;
        use serde_json::json;

        // Create existing note.
        let raw = "\u{feff}---\ntitle: \"My day\"\nsubtitle: \"Note\"\n---\nBody";
        let content = ContentString::from(raw.to_string());
        assert!(!content.is_empty());
        assert!(!content.borrow_dependent().header.is_empty());

        let front_matter = FrontMatter::try_from(content.header()).unwrap();
        assert_eq!(front_matter.get("title"), Some(&json!("My day")));
        assert_eq!(front_matter.get("subtitle"), Some(&json!("Note")));
    }

    #[test]
    fn test_assert_preconditions() {
        // Check `tmpl.filter.assert_preconditions` in
        // `tpnote_lib/src/config_default.toml` to understand these tests.
        use crate::front_matter::FrontMatter;
        use serde_json::json;

        //
        // Is empty.
        let input = "";
        let res = FrontMatter::try_from(input).unwrap();
        assert!(matches!(
            res.assert_precoditions(Path::new("does not matter"))
                .unwrap_err(),
            NoteError::FrontMatterFieldMissing { .. }
        ));

        //
        // Ok as long as no other file with that sort-tag exists.
        let input = "# document start
        title: The book
        sort_tag:    123b";
        let res = FrontMatter::try_from(input).unwrap();
        assert!(matches!(
            res.assert_precoditions(Path::new("./03b-test.md")),
            Ok(())
        ));

        //
        // Should not be a compound type.
        let input = "# document start
        title: The book
        sort_tag:
        -    1234
        -    456";
        let res = FrontMatter::try_from(input).unwrap();
        assert!(matches!(
            res.assert_precoditions(Path::new("does not matter"))
                .unwrap_err(),
            NoteError::FrontMatterFieldIsCompound { .. }
        ));

        //
        // Should not be a compound type.
        let input = "# document start
        title: The book
        sort_tag:
          first:  1234
          second: 456";
        let res = FrontMatter::try_from(input).unwrap();
        assert!(matches!(
            res.assert_precoditions(Path::new("does not matter"))
                .unwrap_err(),
            NoteError::FrontMatterFieldIsCompound { .. }
        ));

        //
        // Not registered file extension.
        let input = "# document start
        title: The book
        file_ext:    xyz";
        let res = FrontMatter::try_from(input).unwrap();
        assert!(matches!(
            res.assert_precoditions(Path::new("does not matter"))
                .unwrap_err(),
            NoteError::FrontMatterFieldIsNotTpnoteExtension { .. }
        ));

        //
        // Check `bool`
        let input = "# document start
        title: The book
        filename_sync: error, here should be a bool";
        let res = FrontMatter::try_from(input).unwrap();
        assert!(matches!(
            res.assert_precoditions(Path::new("does not matter"))
                .unwrap_err(),
            NoteError::FrontMatterFieldIsNotBool { .. }
        ));

        //
        let input = "# document start
        title: my title
        subtitle: my subtitle
        ";
        let expected = json!({"title": "my title", "subtitle": "my subtitle"});
        let expected = expected.as_object().unwrap();

        let output = FrontMatter::try_from(input).unwrap();
        assert_eq!(&output.0, expected);

        let input = "# document start
        title: my title
        file_ext: ''
        ";

        //
        let expected = json!({"title": "my title", "file_ext": ""});
        let expected = expected.as_object().unwrap();

        let output = FrontMatter::try_from(input).unwrap();
        assert_eq!(&output.0, expected);

        //
        let input = "# document start
        title: ''
        subtitle: my subtitle
        ";
        let output = FrontMatter::try_from(input).unwrap();
        assert!(matches!(
            output
                .assert_precoditions(Path::new("does not matter"))
                .unwrap_err(),
            NoteError::FrontMatterFieldIsEmptyString { .. }
        ));

        //
        let input = "# document start
        title: My doc
        author: 
        - First author
        - Second author
        ";
        let output = FrontMatter::try_from(input).unwrap();
        assert!(output
            .assert_precoditions(Path::new("does not matter"))
            .is_ok());

        //
        let input = "# document start
        title: My doc
        subtitle: my subtitle
        author:
        - First title
        - 1234
        ";
        let output = FrontMatter::try_from(input).unwrap();
        assert!(matches!(
            output
                .assert_precoditions(Path::new("does not matter"))
                .unwrap_err(),
            NoteError::FrontMatterFieldIsNotString { .. }
        ));
    }

    #[test]
    fn test_all_leaves() {
        use super::all_leaves;

        let input = json!({
             "first": "tmp: test",
             "second": [
                 "string(a)",
                 "string(b)"
             ],});
        assert!(all_leaves(&input, &|v| matches!(v, Value::String(..))));

        let input = json!({
            "first": "tmp: test",
            "second": [
                1234,
                "string(b)"
            ],});
        assert!(!all_leaves(&input, &|v| matches!(v, Value::String(..))));

        let input = json!({
             "first": "tmp: test",
             "second": [
                 "string(a)",
                 false
             ],});
        assert!(!all_leaves(&input, &|v| matches!(v, Value::String(..))));

        let input = json!({
            "first": "tmp: test",
            "second": [
                "string(a)",
                "string(b)"
            ],});
        assert!(all_leaves(&input, &|v| matches!(v, Value::String(..))
            && v.as_str() != Some("")));

        let input = json!({
             "first": "tmp: test",
             "second": [
                 "string(a)",
                 ""
             ],});
        assert!(!all_leaves(&input, &|v| matches!(v, Value::String(..))
            && v.as_str() != Some("")));
    }
}
