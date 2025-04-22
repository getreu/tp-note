//! Creates a memory representation of the note's YAML header.
//! In this documentation, the terms “YAML header”, ”header” and ”front matter"
//! are used as synonyms for the note's meta data block at the beginning
//! of the text file. Technically this is a wrapper around a `tera::Map`.
use crate::error::NoteError;
use crate::error::FRONT_MATTER_ERROR_MAX_LINES;
use std::ops::Deref;
use std::ops::DerefMut;
use std::str;
use tera::Value;

#[derive(Debug, Eq, PartialEq)]
/// Represents the front matter of the note. This is a newtype
/// for `tera::Map<String, tera::Value>`.
pub struct FrontMatter(pub tera::Map<String, tera::Value>);

/// Helper function asserting that all the leaves of `val` have a certain type.
/// The first parameter is the type to check recursively.
/// The second is a closure that evaluates to true or false.
pub(crate) fn all_leaves(val: &Value, f: &dyn Fn(&Value) -> bool) -> bool {
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
    use crate::config::TMPL_VAR_FM_ALL;
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

        let input1 = Context::from(Path::new("a/b/test.md")).unwrap();
        let input2 = FrontMatter(tmp);

        let mut expected = Context::from(Path::new("a/b/test.md")).unwrap();
        tmp2.remove("fm_numbers");
        tmp2.insert("fm_numbers".to_string(), json!([1, 3, 5])); // String()!
        let tmp2 = tera::Value::from(tmp2);
        expected.insert(TMPL_VAR_FM_ALL, &tmp2); // Map()
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
