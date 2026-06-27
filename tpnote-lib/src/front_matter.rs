//! Creates a memory representation of the note's YAML header.
//! In this documentation, the terms "YAML header", "header" and "front matter"
//! are used as synonyms for the note's meta data block at the beginning
//! of the text file. Technically this is a wrapper around a `serde_json::Map`.
use crate::error::FRONT_MATTER_ERROR_MAX_LINES;
use crate::error::NoteError;
use std::ops::Deref;
use std::ops::DerefMut;
use std::str;

#[derive(Debug, Eq, PartialEq)]
/// Represents the front matter of the note. This is a newtype
/// for `serde_json::Map<String, serde_json::Value>`.
pub struct FrontMatter(pub serde_json::Map<String, serde_json::Value>);

/// Helper function asserting that all the leaves of `val` have a certain type.
/// The first parameter is the type to check recursively.
/// The second is a closure that evaluates to true or false.
#[allow(dead_code)]
pub(crate) fn all_leaves(val: &serde_json::Value, f: &dyn Fn(&serde_json::Value) -> bool) -> bool {
    match &val {
        serde_json::Value::Array(a) => {
            for i in a.iter() {
                if !all_leaves(i, &f) {
                    return false;
                }
            }
        }
        serde_json::Value::Object(map) => {
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

impl TryFrom<&str> for FrontMatter {
    type Error = NoteError;
    /// Helper function deserializing the front-matter of the note file.
    /// An empty header leads to an empty map; no error.
    fn try_from(header: &str) -> Result<FrontMatter, NoteError> {
        let map: serde_json::Map<String, serde_json::Value> =
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

/// Auto dereferences for convenient access to `serde_json::Map`.
impl Deref for FrontMatter {
    type Target = serde_json::Map<String, serde_json::Value>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

/// Auto dereferences for convenient access to `serde_json::Map`.
impl DerefMut for FrontMatter {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

#[cfg(test)]
mod tests {
    use crate::config::TMPL_VAR_FM_ALL;
    use crate::front_matter::FrontMatter;
    use serde_json::json;
    use serde_json::Value;

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

        let mut expected = serde_json::Map::new();
        expected.insert("title".to_string(), json!("The book"));
        expected.insert("subtitle".to_string(), json!("you always wanted"));
        expected.insert("author".to_string(), json!("It\'s me"));
        expected.insert("date".to_string(), json!("2020-04-21"));
        expected.insert("lang".to_string(), json!("en"));
        expected.insert("revision".to_string(), json!("1.0"));
        expected.insert("sort_tag".to_string(), json!("20200420-21_22"));
        expected.insert("file_ext".to_string(), json!("md"));
        expected.insert("height".to_string(), json!(1.23));
        expected.insert("count".to_string(), json!(2));
        expected.insert("neg".to_string(), json!(-1));
        expected.insert("flag".to_string(), json!(true));
        expected.insert("numbers".to_string(), json!([1, 3, 5]));

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

        let mut tmp = serde_json::Map::new();
        tmp.insert("file_ext".to_string(), json!("md"));
        tmp.insert("height".to_string(), json!(1.23));
        tmp.insert("count".to_string(), json!(2));
        tmp.insert("neg".to_string(), json!(-1));
        tmp.insert("flag".to_string(), json!(true));
        tmp.insert("numbers".to_string(), json!([1, 3, 5]));

        let input1 = Context::from(Path::new("a/b/test.md")).unwrap();
        let input2 = FrontMatter(tmp);

        let mut expected = Context::from(Path::new("a/b/test.md")).unwrap();
        let tmp2 = Value::from_serializable(&json!({
            "fm_file_ext": "md",
            "fm_height": 1.23,
            "fm_count": 2,
            "fm_neg": -1,
            "fm_flag": true,
            "fm_numbers": [1, 3, 5]
        }));
        expected.insert(TMPL_VAR_FM_ALL, &tmp2);
        let expected = expected.insert_front_matter(&FrontMatter::try_from("").unwrap());

        let result = input1.insert_front_matter(&input2);

        assert_eq!(result, expected);
    }

    #[test]
    fn test_try_from_content() {
        use crate::content::Content;
        use crate::content::ContentString;
        use serde_json::json;

        // Create existing note.
        let raw = "\u{feff}---\ntitle: \"My day\"\nsubtitle: \"Note\"\n---\nBody";
        let content = ContentString::from_string(raw.to_string(), "doc".to_string());
        assert!(!content.is_empty());
        assert!(!content.borrow_dependent().header.is_empty());

        let front_matter = FrontMatter::try_from(content.header()).unwrap();
        assert_eq!(front_matter.get("title"), Some(&json!("My day")));
        assert_eq!(front_matter.get("subtitle"), Some(&json!("Note")));
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
