//! Extends the built-in Tera filters.
use crate::config::FILENAME_DOTFILE_MARKER;
use crate::config::LIB_CFG;
use crate::filename::NotePath;
use crate::filename::NotePathBuf;
#[cfg(feature = "lang-detection")]
use crate::settings::FilterGetLang;
use crate::settings::SETTINGS;
use lazy_static::lazy_static;
#[cfg(feature = "lang-detection")]
use lingua::{LanguageDetector, LanguageDetectorBuilder};
use parse_hyperlinks::iterator::first_hyperlink;
use sanitize_filename_reader_friendly::sanitize;
use std::borrow::Cow;
use std::collections::HashMap;
use std::hash::BuildHasher;
use std::path::Path;
use std::path::PathBuf;
use tera::{to_value, try_get_value, Result as TeraResult, Tera, Value};

/// Filter parameter of the `cut_filter()` limiting the maximum length of
/// template variables. The filter is usually used to in the note's front matter
/// as title. For example: the title should not be too long, because it will end
/// up as part of the filename when the note is saved to disk. Filenames of some
/// operating systems are limited to 255 bytes.
#[cfg(not(test))]
const CUT_LEN_MAX: usize = 200;
#[cfg(test)]
pub const CUT_LEN_MAX: usize = 10;

lazy_static! {
/// Tera object with custom functions registered.
    pub static ref TERA: Tera = {
        let mut tera = Tera::default();
        tera.register_filter("to_yaml", to_yaml_filter);
        tera.register_filter("to_html", to_html_filter);
        tera.register_filter("sanit", sanit_filter);
        tera.register_filter("link_text", link_text_filter);
        tera.register_filter("link_dest", link_dest_filter);
        tera.register_filter("link_title", link_title_filter);
        tera.register_filter("heading", heading_filter);
        tera.register_filter("cut", cut_filter);
        tera.register_filter("trim_file_sort_tag", trim_file_sort_tag_filter);
        tera.register_filter("file_sort_tag", file_sort_tag_filter);
        tera.register_filter("file_stem", file_stem_filter);
        tera.register_filter("file_copy_counter", file_copy_counter_filter);
        tera.register_filter("file_name", file_name_filter);
        tera.register_filter("file_ext", file_ext_filter);
        tera.register_filter("prepend", prepend_filter);
        tera.register_filter("append", append_filter);
        tera.register_filter("field", field_filter);
        tera.register_filter("get_lang", get_lang_filter);
        tera.register_filter("map_lang", map_lang_filter);
        tera
    };
}

/// A filter converting an input `tera::Value::Object` into a
/// `tera::Value::String(s)` with `s` being the YAML representation of the
/// object. When the optional parameter `key='k'` is given, the input can be
/// any `tera::Value` variant.
/// The optional parameter `tab=n` indents the YAML values `n` characters to
/// the right of the first character of the key by inserting additional spaces
/// between the key and the value. When `tab=n` is given, it has precendence
/// over the  default value, read from the configuration file variable
/// `tmpl.filter_to_yaml_tab`.
fn to_yaml_filter<S: BuildHasher>(
    val: &Value,
    args: &HashMap<String, Value, S>,
) -> TeraResult<Value> {
    let val_yaml = if let Some(Value::String(k)) = args.get("key") {
        let mut m = tera::Map::new();
        m.insert(k.to_owned(), val.to_owned());
        serde_yaml::to_string(&m).unwrap()
    } else {
        serde_yaml::to_string(&val).unwrap()
    };

    // Translate the empty set, into an empty string and return it.
    if val_yaml.trim_end() == "{}" {
        return Ok(tera::Value::String("".to_string()));
    }

    // Formatting: adjust indent.
    let val_yaml: String = if let Some(n) = args.get("tab").and_then(|v| v.as_u64()).or_else(|| {
        let lib_cfg = LIB_CFG.read_recursive();
        let n = lib_cfg.tmpl.filter_to_yaml_tab;
        if n == 0 {
            None
        } else {
            Some(n)
        }
    }) {
        val_yaml
            .lines()
            .map(|l| {
                let mut colon_pos = 0;
                let mut insert = 0;
                if let Some(colpos) = l.find(": ") {
                    colon_pos = colpos;
                    if let Some(key_pos) = l.find(char::is_alphabetic) {
                        if key_pos < colon_pos
                            && !l.find('\'').is_some_and(|p| p < colon_pos)
                            && !l.find("\"'").is_some_and(|p| p < colon_pos)
                        {
                            insert = (n as usize).saturating_sub(colon_pos + ": ".len());
                        }
                    }
                };

                // Enlarge indent.
                let mut l = l.to_owned();
                let strut = std::iter::repeat(' ').take(insert).collect::<String>();
                // If `insert>0`, we know that `colon_pos>0`.
                // `colon_pos+1` inserts between `: `.
                l.insert_str(colon_pos + 1, &strut);
                l.push('\n');
                l
            })
            .collect::<String>()
    } else {
        val_yaml
    };

    Ok(tera::Value::String(val_yaml.trim_end().to_string()))
}

/// A filter that coverts a `tera::Value` tree into an HTML representation,
/// with following HTLM tags:
/// * `Value::Object`: `<blockquote class="fm">` and `<div class="fm">`,
/// * `Value::Array`: `<ul class="fm">` and `<li class="fm">`,
/// * `Value::String`: no tag,
/// * Other non-string basic types: `<code class="fm">`.
fn to_html_filter<S: BuildHasher>(
    value: &Value,
    _args: &HashMap<String, Value, S>,
) -> TeraResult<Value> {
    fn tag_to_html(val: Value, output: &mut String) {
        match val {
            Value::Array(a) => {
                output.push_str("<ul class=\"fm\">");
                for i in a {
                    output.push_str("<li class=\"fm\">");
                    tag_to_html(i, output);
                    output.push_str("</li>");
                }
                output.push_str("</ul>");
            }

            Value::String(s) => output.push_str(&s),

            Value::Object(map) => {
                output.push_str("<blockquote class=\"fm\">");
                for (k, v) in map {
                    output.push_str("<div class=\"fm\">");
                    output.push_str(&k);
                    output.push_str(": ");
                    tag_to_html(v, output);
                    output.push_str("</div>");
                }
                output.push_str("</blockquote>");
            }

            _ => {
                output.push_str("<code class=\"fm\">");
                output.push_str(&val.to_string());
                output.push_str("</code>");
            }
        };
    }

    let val = try_get_value!("to_yaml", "value", Value, value);

    let mut html = String::new();
    tag_to_html(val, &mut html);

    Ok(tera::Value::String(html.to_string()))
}

/// Adds a new filter to Tera templates:
/// `sanit` or `sanit()` sanitizes a string so that it can be used to
/// assemble filenames or paths. In addition, `sanit(alpha=true)` prepends
/// the `sort_tag_extra_separator` when the result starts with one of
/// `sort_tag_chars`, usually a number. This way we guaranty that the filename
/// never starts with a number. We do not allow this, to be able to distinguish
/// reliably the sort tag from the filename. In addition to the above, the
/// filter checks if the string represents a "well-formed" filename. If it
/// is the case, and the filename starts with a dot, the file is prepended by
/// `sort_tag_extra_separator`. Note, this filter converts all input types to
/// `tera::String`.
fn sanit_filter<S: BuildHasher>(p: &Value, _args: &HashMap<String, Value, S>) -> TeraResult<Value> {
    // Take unmodified `String()`, but format all other types into
    // string.
    let mut p = if p.is_string() {
        Cow::Borrowed(p.as_str().unwrap())
    } else {
        // Convert and format.
        Cow::Owned(p.to_string())
    };

    // Check if this is a usual dotfile filename.
    let is_dotfile =
        p.starts_with(FILENAME_DOTFILE_MARKER) && PathBuf::from(&*p).has_wellformed_filename();

    // Sanitize string.
    p = sanitize(&p).into();

    // If `FILNAME_DOTFILE_MARKER` was stripped, prepend one.
    if is_dotfile && !p.starts_with(FILENAME_DOTFILE_MARKER) {
        let mut s = String::from(FILENAME_DOTFILE_MARKER);
        s.push_str(&p);
        p = Cow::from(s);
    }

    Ok(to_value(&p)?)
}

/// A Tera filter that searches for the first Markdown or reStructuredText link
/// in the input stream and returns the link's name.
fn link_text_filter<S: BuildHasher>(
    value: &Value,
    _args: &HashMap<String, Value, S>,
) -> TeraResult<Value> {
    let p = try_get_value!("link_text", "value", String, value);

    let hyperlink = Hyperlink::from(&p).unwrap_or_default();

    Ok(to_value(hyperlink.name)?)
}

/// A Tera filter that searches for the first Markdown or reStructuredText link
/// in the input stream and returns the link's URL.
fn link_dest_filter<S: BuildHasher>(
    value: &Value,
    _args: &HashMap<String, Value, S>,
) -> TeraResult<Value> {
    let p = try_get_value!("link_text", "value", String, value);

    let hyperlink = Hyperlink::from(&p).unwrap_or_default();

    Ok(to_value(hyperlink.target)?)
}

/// A Tera filter that searches for the first Markdown or reStructuredText link
/// in the input stream and returns the link's title.
fn link_title_filter<S: BuildHasher>(
    value: &Value,
    _args: &HashMap<String, Value, S>,
) -> TeraResult<Value> {
    let p = try_get_value!("link_title", "value", String, value);

    let hyperlink = Hyperlink::from(&p).unwrap_or_default();

    Ok(to_value(hyperlink.title)?)
}

/// A Tera filter that truncates the input stream and returns the
/// max `CUT_LEN_MAX` bytes of valid UTF-8.
/// This filter only acts on `String` types. All other types
/// are passed through.
fn cut_filter<S: BuildHasher>(
    value: &Value,
    _args: &HashMap<String, Value, S>,
) -> TeraResult<Value> {
    let p = try_get_value!("cut", "value", tera::Value, value);

    match p {
        tera::Value::String(sv) => {
            let mut short = "";
            for i in (0..CUT_LEN_MAX).rev() {
                if let Some(s) = sv.get(..i) {
                    short = s;
                    break;
                }
            }
            Ok(to_value(short)?)
        }
        _ => Ok(p),
    }
}

/// A Tera filter that returns the first line or the first sentence of the input
/// stream.
fn heading_filter<S: BuildHasher>(
    value: &Value,
    _args: &HashMap<String, Value, S>,
) -> TeraResult<Value> {
    let p = try_get_value!("heading", "value", String, value);
    let p = p.trim_start();

    // Find the first heading, can finish with `. `, `.\n` or `.\r\n` on Windows.
    let mut index = p.len();

    if let Some(i) = p.find(". ") {
        if i < index {
            index = i;
        }
    }
    if let Some(i) = p.find(".\n") {
        if i < index {
            index = i;
        }
    }
    if let Some(i) = p.find(".\r\n") {
        if i < index {
            index = i;
        }
    }
    if let Some(i) = p.find('!') {
        if i < index {
            index = i;
        }
    }
    if let Some(i) = p.find('?') {
        if i < index {
            index = i;
        }
    }
    if let Some(i) = p.find("\n\n") {
        if i < index {
            index = i;
        }
    }
    if let Some(i) = p.find("\r\n\r\n") {
        if i < index {
            index = i;
        }
    }
    let content_heading = p[0..index].to_string();

    Ok(to_value(content_heading)?)
}

/// A Tera filter that takes a path and extracts the tag of the filename.
fn file_sort_tag_filter<S: BuildHasher>(
    value: &Value,
    _args: &HashMap<String, Value, S>,
) -> TeraResult<Value> {
    let p = try_get_value!("file_sort_tag", "value", String, value);
    let p = PathBuf::from(p);
    let (tag, _, _, _, _) = p.disassemble();

    Ok(to_value(tag)?)
}

/// A Tera filter that takes a path and extracts its last element.
/// This function trims the `sort_tag` if present.
fn trim_file_sort_tag_filter<S: BuildHasher>(
    value: &Value,
    _args: &HashMap<String, Value, S>,
) -> TeraResult<Value> {
    let p = try_get_value!("trim_file_sort_tag", "value", String, value);
    let p = PathBuf::from(p);
    let (_, fname, _, _, _) = p.disassemble();

    Ok(to_value(fname)?)
}

/// A Tera filter that takes a path and extracts its file stem,
/// in other words: the filename without `sort_tag`, `file_copy_counter`
/// and `extension`.
fn file_stem_filter<S: BuildHasher>(
    value: &Value,
    _args: &HashMap<String, Value, S>,
) -> TeraResult<Value> {
    let p = try_get_value!("file_stem", "value", String, value);
    let p = PathBuf::from(p);
    let (_, _, stem, _, _) = p.disassemble();

    Ok(to_value(stem)?)
}

/// A Tera filter that takes a path and extracts its copy counter,
/// or, to put it another way: the filename without `sort_tag`, `file_stem`
/// and `file_ext` (and their separators). If the filename contains a
/// `copy_counter=n`, the retured JSON value variant is `Value::Number(n)`.
/// Otherwise it is `Value::Null`.
fn file_copy_counter_filter<S: BuildHasher>(
    value: &Value,
    _args: &HashMap<String, Value, S>,
) -> TeraResult<Value> {
    let p = try_get_value!("file_copy_counter", "value", String, value);
    let p = PathBuf::from(p);
    let (_, _, _, copy_counter, _) = p.disassemble();
    let copy_counter = match copy_counter {
        Some(cc) => to_value(cc)?,
        None => Value::Null,
    };

    Ok(copy_counter)
}

/// A Tera filter that takes a path and extracts its filename without
/// file extension. The filename may contain a sort-tag, a copy-counter and
/// also separators.
fn file_name_filter<S: BuildHasher>(
    value: &Value,
    _args: &HashMap<String, Value, S>,
) -> TeraResult<Value> {
    let p = try_get_value!("file_name", "value", String, value);

    let filename = Path::new(&p)
        .file_name()
        .unwrap_or_default()
        .to_str()
        .unwrap_or_default();

    Ok(to_value(filename)?)
}

/// A Tera filter that prepends the string parameter `with`, but only if the
/// input stream is not empty.
/// When called with the strings parameter `with_sort_tag`, the filter
/// prepends the sort-tag and all necessary sort-tag separator characters,
/// regardless whether the input stream in empty or not.
fn prepend_filter<S: BuildHasher>(
    value: &Value,
    args: &HashMap<String, Value, S>,
) -> TeraResult<Value> {
    let input = try_get_value!("prepend", "value", String, value);

    let mut res = input;
    if let Some(Value::String(with)) = args.get("with") {
        let mut s = String::new();
        if !res.is_empty() {
            s.push_str(with);
            s.push_str(&res);
            res = s;
        };
    } else if let Some(Value::String(sort_tag)) = args.get("with_sort_tag") {
        res = PathBuf::from_disassembled(sort_tag, &res, None, "")
            .to_str()
            .unwrap_or_default()
            .to_string();
    };

    Ok(to_value(res)?)
}

/// A Tera filter that appends the string parameter `with`. In addition, the
/// flag `newline` inserts a newline character at end of the result. In
/// case the input stream is empty, nothing is appended.
fn append_filter<S: BuildHasher>(
    value: &Value,
    args: &HashMap<String, Value, S>,
) -> TeraResult<Value> {
    let input = try_get_value!("append", "value", String, value);

    if input.is_empty() {
        return Ok(Value::String("".to_string()));
    }

    let mut res = input.clone();
    if let Some(Value::String(with)) = args.get("with") {
        res.push_str(with);
    };

    if let Some(Value::Bool(newline)) = args.get("newline") {
        if *newline {
            #[cfg(not(target_family = "windows"))]
            res.push('\n');
            #[cfg(target_family = "windows")]
            res.push_str("\r\n");
        }
    };

    Ok(to_value(res)?)
}

/// A Tera filter that takes a path and extracts its file extension.
fn file_ext_filter<S: BuildHasher>(
    value: &Value,
    _args: &HashMap<String, Value, S>,
) -> TeraResult<Value> {
    let p = try_get_value!("file_ext", "value", String, value);

    let ext = Path::new(&p)
        .extension()
        .unwrap_or_default()
        .to_str()
        .unwrap_or_default();

    Ok(to_value(ext)?)
}

/// A Tera filter that takes a list of variables and removes
/// one.
fn field_filter<S: BuildHasher>(
    value: &Value,
    args: &HashMap<String, Value, S>,
) -> TeraResult<Value> {
    let mut map = try_get_value!("field", "value", tera::Map<String, tera::Value>, value);

    if let Some(outkey) = args.get("out") {
        let outkey = try_get_value!("field", "out", String, outkey);
        let _ = map.remove(outkey.trim_start_matches("fm_"));
    };

    if let Some(inkey) = args.get("in") {
        let inkey = try_get_value!("field", "in", String, inkey);
        let inval = args
            .get("inval")
            .map(|v| v.to_owned())
            .unwrap_or(tera::Value::Null);
        map.insert(inkey.trim_start_matches("fm_").to_string(), inval);
    };

    Ok(to_value(&map).unwrap_or_default())
}

/// A Tera filter telling which natural language some provided textual data is
/// written in. It returns the ISO 639-1 code representations of the detected
/// language. This filter only acts on `String` types. All other types are
/// passed through. Returns the empty string in case the language can not be
/// detected reliably.
#[cfg(feature = "lang-detection")]
fn get_lang_filter<S: BuildHasher>(
    value: &Value,
    _args: &HashMap<String, Value, S>,
) -> TeraResult<Value> {
    let p = try_get_value!("get_lang", "value", tera::Value, value);
    match p {
        #[allow(unused_variables)]
        tera::Value::String(input) => {
            let input = input.trim();
            // Return early if there is no input text.
            if input.is_empty() {
                return Ok(to_value("").unwrap());
            }

            let settings = SETTINGS.read_recursive();
            let detector: LanguageDetector = match &settings.filter_get_lang {
                FilterGetLang::SomeLanguages(iso_codes) => {
                    log::trace!(
                        "Execute template filter `get_lang` \
                        with languages candiates: {:?}",
                        iso_codes,
                    );
                    LanguageDetectorBuilder::from_iso_codes_639_1(iso_codes)
                }
                FilterGetLang::AllLanguages => {
                    log::trace!(
                        "Execute template filter `get_lang` \
                        with all available languages",
                    );
                    LanguageDetectorBuilder::from_all_languages()
                }
                FilterGetLang::Error(e) => return Err(tera::Error::from(e.to_string())),
                _ => return Ok(to_value("").unwrap()),
            }
            .build();

            let detected_language = detector
                .detect_language_of(input)
                .map(|l| format!("{}", l.iso_code_639_1()))
                // If not languages can be detected, this returns the empty
                // string.
                .unwrap_or_default();
            log::debug!("Language '{}' in input detected.", detected_language);

            Ok(to_value(detected_language)?)
        }
        _ => Ok(p),
    }
}

#[cfg(not(feature = "lang-detection"))]
fn get_lang_filter<S: BuildHasher>(
    _value: &Value,
    _args: &HashMap<String, Value, S>,
) -> TeraResult<Value> {
    Ok(to_value("").unwrap())
}

/// A mapper for ISO 639 codes adding some region information, e.g.
/// `en` to `en-US` or `de` to `de-DE`. Configure the mapping with
/// `tmpl.filter_map_lang`.
/// An input value without mapping definition is passed through.
/// When the optional parameter `default` is given, e.g.
/// `map_lang(default=val)`, an empty input string is mapped to `val`.  
fn map_lang_filter<S: BuildHasher>(
    value: &Value,
    args: &HashMap<String, Value, S>,
) -> TeraResult<Value> {
    let p = try_get_value!("map_lang", "value", tera::Value, value);

    match p {
        tera::Value::String(input) => {
            let input = input.trim();
            if input.is_empty() {
                if let Some(val) = args.get("default") {
                    return Ok(to_value(val)?);
                } else {
                    return Ok(to_value("")?);
                };
            };
            let settings = SETTINGS.read_recursive();
            if let Some(btm) = &settings.filter_map_lang_btmap {
                match btm.get(input) {
                    None => Ok(to_value(input)?),
                    Some(tag) => Ok(to_value(tag)?),
                }
            } else {
                Ok(to_value(input)?)
            }
        }
        _ => Ok(p),
    }
}

#[derive(Debug, Eq, PartialEq, Default)]
/// Represents a hyperlink.
struct Hyperlink {
    name: String,
    target: String,
    title: String,
}

impl Hyperlink {
    /// Parse a markdown formatted hyperlink and stores the result in `Self`.
    fn from(input: &str) -> Option<Hyperlink> {
        first_hyperlink(input).map(|(link_name, link_target, link_title)| Hyperlink {
            name: link_name.to_string(),
            target: link_target.to_string(),
            title: link_title.to_string(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use parking_lot::RwLockWriteGuard;
    use serde_json::json;
    use std::collections::{BTreeMap, HashMap};
    use tera::to_value;

    #[test]
    fn test_to_yaml_filter() {
        // No key, the input is of type `Value::Object()`.
        let mut input = tera::Map::new();
        input.insert("number_type".to_string(), json!(123));

        let expected = "number_type:  123".to_string();

        let args = HashMap::new();
        assert_eq!(
            to_yaml_filter(&Value::Object(input), &args).unwrap(),
            Value::String(expected)
        );

        //
        // The key is `author`, the value is of type `Value::String()`.
        let input = "Getreu".to_string();

        let expected = "author:       Getreu".to_string();

        let mut args = HashMap::new();
        args.insert("key".to_string(), to_value("author").unwrap());
        assert_eq!(
            to_yaml_filter(&Value::String(input), &args).unwrap(),
            Value::String(expected)
        );

        //
        // The key is `my`, the value is of type `Value::Object()`.
        let mut input = tera::Map::new();
        input.insert(
            "author".to_string(),
            json!(["Getreu: Noname", "Jens: Noname"]),
        );

        let expected = "my:\n  author:\n  - 'Getreu: Noname'\n  - 'Jens: Noname'".to_string();

        let mut args = HashMap::new();
        args.insert("key".to_string(), to_value("my").unwrap());
        assert_eq!(
            to_yaml_filter(&Value::Object(input), &args).unwrap(),
            Value::String(expected)
        );

        //
        // The key is `my`, the value is of type `Value::Object()`.
        let mut input = tera::Map::new();
        input.insert("number_type".to_string(), json!(123));

        let expected = "my:\n  number_type: 123".to_string();

        let mut args = HashMap::new();
        args.insert("key".to_string(), to_value("my").unwrap());
        assert_eq!(
            to_yaml_filter(&Value::Object(input), &args).unwrap(),
            Value::String(expected)
        );

        //
        // The key is `my`, `tab` is 10, the value is of type `Value::Object()`.
        let mut input = tera::Map::new();
        input.insert("num".to_string(), json!(123));

        let expected = "my:\n  num:    123".to_string();

        let mut args = HashMap::new();
        args.insert("key".to_string(), to_value("my").unwrap());
        args.insert("tab".to_string(), to_value(10).unwrap());
        assert_eq!(
            to_yaml_filter(&Value::Object(input), &args).unwrap(),
            Value::String(expected)
        );

        //
        // Empty input.
        let input = tera::Map::new();

        let expected = "".to_string();

        let mut args = HashMap::new();
        args.insert("tab".to_string(), to_value(10).unwrap());
        assert_eq!(
            to_yaml_filter(&Value::Object(input), &args).unwrap(),
            Value::String(expected)
        );

        //
        // Empty input with key.
        let input = tera::Map::new();

        let expected = "my:       {}".to_string();

        let mut args = HashMap::new();
        args.insert("key".to_string(), to_value("my").unwrap());
        args.insert("tab".to_string(), to_value(10).unwrap());
        assert_eq!(
            to_yaml_filter(&Value::Object(input), &args).unwrap(),
            Value::String(expected)
        );

        //
        // Simple input string, no map.
        let input = json!("my str");
        let expected = "my str".to_string();
        let mut args = HashMap::new();
        args.insert("tab".to_string(), to_value(10).unwrap());
        assert_eq!(
            to_yaml_filter(&input, &args).unwrap(),
            Value::String(expected)
        );

        //
        // Simple input string, no map.
        let input = json!("my: str");
        let expected = "'my: str'".to_string();
        let mut args = HashMap::new();
        args.insert("tab".to_string(), to_value(10).unwrap());
        assert_eq!(
            to_yaml_filter(&input, &args).unwrap(),
            Value::String(expected)
        );

        //
        // Simple input number, no map.
        let input = json!(9876);
        let expected = "9876".to_string();
        let mut args = HashMap::new();
        args.insert("tab".to_string(), to_value(10).unwrap());
        assert_eq!(
            to_yaml_filter(&input, &args).unwrap(),
            Value::String(expected)
        );
    }
    #[test]
    fn test_to_html_filter() {
        //
        let input = json!(["Hello", "World", 123]);
        let expected = "<ul class=\"fm\"><li class=\"fm\">Hello</li>\
            <li class=\"fm\">World</li><li class=\"fm\">\
            <code class=\"fm\">123</code></li></ul>"
            .to_string();

        let args = HashMap::new();
        assert_eq!(
            to_html_filter(&input, &args).unwrap(),
            Value::String(expected)
        );

        //
        let input = json!({
            "title": "tmp: test",
            "subtitle": "Note",
            "author": [
                "Getreu: Noname",
                "Jens: Noname"
            ],
            "date": "2023-09-12T00:00:00.000Z",
            "my": {
                "num_type": 123,
                "str_type": {
                    "sub1": "foo",
                    "sub2": "bar"
                },
                "weiter": 3454
            },
            "other": "my \"new\" text",
            "filename_sync": false,
            "lang": "et-ET"
        });
        let expected = "<blockquote class=\"fm\">\
            <div class=\"fm\">author: <ul class=\"fm\">\
            <li class=\"fm\">Getreu: Noname</li>\
            <li class=\"fm\">Jens: Noname</li></ul></div>\
            <div class=\"fm\">date: 2023-09-12T00:00:00.000Z</div>\
            <div class=\"fm\">filename_sync: <code class=\"fm\">false</code></div>\
            <div class=\"fm\">lang: et-ET</div>\
            <div class=\"fm\">my: \
              <blockquote class=\"fm\">\
              <div class=\"fm\">num_type: <code class=\"fm\">123</code></div>\
              <div class=\"fm\">str_type: \
                <blockquote class=\"fm\"><div class=\"fm\">sub1: foo</div>\
                <div class=\"fm\">sub2: bar</div></blockquote></div>\
                <div class=\"fm\">weiter: <code class=\"fm\">3454</code></div>\
                </blockquote></div>\
            <div class=\"fm\">other: my \"new\" text</div>\
            <div class=\"fm\">subtitle: Note</div>\
            <div class=\"fm\">title: tmp: test</div>\
            </blockquote>"
            .to_string();

        let args = HashMap::new();
        assert_eq!(
            to_html_filter(&input, &args).unwrap(),
            Value::String(expected)
        );
    }

    #[test]
    fn test_sanit_filter() {
        let result = sanit_filter(
            &to_value(".# Strange filename? Yes.").unwrap(),
            &HashMap::new(),
        );
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), to_value("Strange filename_ Yes").unwrap());

        let result = sanit_filter(&to_value("Correct filename.pdf").unwrap(), &HashMap::new());
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), to_value("Correct filename.pdf").unwrap());

        let result = sanit_filter(&to_value(".dotfilename").unwrap(), &HashMap::new());
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), to_value(".dotfilename").unwrap());
    }

    #[test]
    fn test_field_filter() {
        //
        let input = json!({"title": "my title", "subtitle": "my subtitle"});
        let mut args = HashMap::new();
        args.insert("out".to_string(), to_value("fm_title").unwrap());
        let expected = json!({"subtitle": "my subtitle"});
        let result = field_filter(&input, &args);
        //eprintln!("{:?}", result);
        assert_eq!(result.unwrap(), expected);

        //
        let input = json!({"title": "my title", "subtitle": "my subtitle"});
        let mut args = HashMap::new();
        args.insert("out".to_string(), to_value("title").unwrap());
        let expected = json!({"subtitle": "my subtitle"});
        let result = field_filter(&input, &args);
        //eprintln!("{:?}", result);
        assert_eq!(result.unwrap(), expected);

        //
        let input = json!({"title": "my title", "subtitle": "my subtitle"});
        let mut args = HashMap::new();
        args.insert("out".to_string(), to_value("fm_title").unwrap());
        args.insert("in".to_string(), to_value("fm_new").unwrap());
        args.insert("inval".to_string(), to_value("my new").unwrap());
        let expected = json!({"new": "my new", "subtitle": "my subtitle"});
        let result = field_filter(&input, &args);
        //eprintln!("{:?}", result);
        assert_eq!(result.unwrap(), expected);

        //
        let input = json!({"title": "my title", "subtitle": "my subtitle"});
        let mut args = HashMap::new();
        args.insert("in".to_string(), to_value("fm_title").unwrap());
        args.insert("inval".to_string(), to_value("my replaced title").unwrap());
        let expected = json!({"title": "my replaced title", "subtitle": "my subtitle"});
        let result = field_filter(&input, &args);
        //eprintln!("{:?}", result);
        assert_eq!(result.unwrap(), expected);

        //
        let input = json!({"title": "my title"});
        let mut args = HashMap::new();
        args.insert("in".to_string(), to_value("fm_new").unwrap());
        let expected = json!({"new": null, "title": "my title"});
        let result = field_filter(&input, &args);
        //eprintln!("{:?}", result);
        assert_eq!(result.unwrap(), expected);

        //
        let input = json!({"title": "my title"});
        let mut args = HashMap::new();
        args.insert("in".to_string(), to_value("fm_new").unwrap());
        args.insert("inval".to_string(), to_value("my new").unwrap());
        let expected = json!({"new": "my new", "title": "my title"});
        let result = field_filter(&input, &args);
        //eprintln!("{:?}", result);
        assert_eq!(result.unwrap(), expected);
    }

    #[test]
    fn test_prepend_filter() {
        // `with`
        let mut args = HashMap::new();
        args.insert("with".to_string(), to_value("-").unwrap());
        let result = prepend_filter(&to_value("1. My first chapter").unwrap(), &args);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), to_value("-1. My first chapter").unwrap());

        let mut args = HashMap::new();
        args.insert("with".to_string(), to_value("_").unwrap());
        let result = prepend_filter(&to_value("").unwrap(), &args);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), to_value("").unwrap());

        // `with_sort_tag`
        let mut args = HashMap::new();
        args.insert("with_sort_tag".to_string(), to_value("20230809").unwrap());
        let result = prepend_filter(&to_value("1. My first chapter").unwrap(), &args);
        assert!(result.is_ok());
        assert_eq!(
            result.unwrap(),
            to_value("20230809-1. My first chapter").unwrap()
        );

        let mut args = HashMap::new();
        args.insert("with_sort_tag".to_string(), to_value("20230809").unwrap());
        let result = prepend_filter(&to_value("1-My first chapter").unwrap(), &args);
        assert!(result.is_ok());
        assert_eq!(
            result.unwrap(),
            to_value("20230809-'1-My first chapter").unwrap()
        );

        let mut args = HashMap::new();
        args.insert("with_sort_tag".to_string(), to_value("").unwrap());
        let result = prepend_filter(&to_value("1. My first chapter").unwrap(), &args);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), to_value("1. My first chapter").unwrap());

        let mut args = HashMap::new();
        args.insert("with_sort_tag".to_string(), to_value("").unwrap());
        let result = prepend_filter(&to_value("1-My first chapter").unwrap(), &args);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), to_value("'1-My first chapter").unwrap());

        let mut args = HashMap::new();
        args.insert("with_sort_tag".to_string(), to_value("20230809").unwrap());
        let result = prepend_filter(&to_value("").unwrap(), &args);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), to_value("20230809-'").unwrap());

        let mut args = HashMap::new();
        args.insert("with_sort_tag".to_string(), to_value("").unwrap());
        let result = prepend_filter(&to_value("").unwrap(), &args);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), to_value("'").unwrap());
    }

    #[test]
    fn test_append_filter() {
        // `with`
        let mut args = HashMap::new();
        args.insert("with".to_string(), to_value("-").unwrap());
        let result = append_filter(&to_value("1. My first chapter").unwrap(), &args);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), to_value("1. My first chapter-").unwrap());

        let mut args = HashMap::new();
        args.insert("with".to_string(), to_value("_").unwrap());
        let result = append_filter(&to_value("").unwrap(), &args);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), to_value("").unwrap());

        // `with_sort_tag`
        let mut args = HashMap::new();
        args.insert("with".to_string(), to_value("-").unwrap());
        args.insert("newline".to_string(), to_value(true).unwrap());
        let result = append_filter(&to_value("1. My first chapter").unwrap(), &args);
        assert!(result.is_ok());
        #[cfg(not(target_family = "windows"))]
        assert_eq!(result.unwrap(), to_value("1. My first chapter-\n").unwrap());
        #[cfg(target_family = "windows")]
        assert_eq!(
            result.unwrap(),
            to_value("1. My first chapter-\r\n").unwrap()
        );
    }

    #[test]
    fn test_link_text_link_dest_link_title_filter() {
        let args = HashMap::new();
        // Test Markdown link in clipboard.
        let input = r#"xxx[Jens Getreu's blog](https://blog.getreu.net "My blog")"#;
        let output_ln = link_text_filter(&to_value(input).unwrap(), &args).unwrap_or_default();
        assert_eq!("Jens Getreu's blog", output_ln);
        let output_lta = link_dest_filter(&to_value(input).unwrap(), &args).unwrap_or_default();
        assert_eq!("https://blog.getreu.net", output_lta);
        let output_lti = link_title_filter(&to_value(input).unwrap(), &args).unwrap_or_default();
        assert_eq!("My blog", output_lti);

        // Test non-link string in clipboard.
        let input = "Tp-Note helps you to quickly get\
            started writing notes.";
        let output_ln = link_text_filter(&to_value(input).unwrap(), &args).unwrap_or_default();
        assert_eq!("", output_ln);
        let output_lta = link_dest_filter(&to_value(input).unwrap(), &args).unwrap_or_default();
        assert_eq!("", output_lta);
        let output_lti = link_title_filter(&to_value(input).unwrap(), &args).unwrap_or_default();
        assert_eq!("", output_lti);
    }

    #[test]
    fn test_cut_filter() {
        let args = HashMap::new();
        // Test Markdown link in clipboard.
        let input = "Jens Getreu's blog";
        let output = cut_filter(&to_value(input).unwrap(), &args).unwrap_or_default();
        assert_eq!("Jens Getr", output);

        let args = HashMap::new();
        // Test Markdown link in clipboard.
        let input = 222; // Number type.
        let output = cut_filter(&to_value(input).unwrap(), &args).unwrap_or_default();
        assert_eq!(222, output);
    }

    #[test]
    fn test_heading_filter() {
        let args = HashMap::new();

        //
        // Test find first sentence.
        let input = "N.ote.\nIt helps. Get quickly\
            started writing notes.";
        let output = heading_filter(&to_value(input).unwrap(), &args).unwrap_or_default();
        // This string is shortened.
        assert_eq!("N.ote", output);

        //
        // Test find first sentence (Windows)
        let input = "N.ote.\r\nIt helps. Get quickly\
            started writing notes.";
        let output = heading_filter(&to_value(input).unwrap(), &args).unwrap_or_default();
        // This string is shortened.
        assert_eq!("N.ote", output);

        //
        // Test find heading
        let input = "N.ote\n\nIt helps. Get quickly\
            started writing notes.";
        let output = heading_filter(&to_value(input).unwrap(), &args).unwrap_or_default();
        // This string is shortened.
        assert_eq!("N.ote", output);

        //
        // Test find heading (Windows)
        let input = "N.ote\r\n\r\nIt helps. Get quickly\
            started writing notes.";
        let output = heading_filter(&to_value(input).unwrap(), &args).unwrap_or_default();
        // This string is shortened.
        assert_eq!("N.ote", output);

        //
        // Test trim whitespace
        let input = "\r\n\r\n  \tIt helps. Get quickly\
            started writing notes.";
        let output = heading_filter(&to_value(input).unwrap(), &args).unwrap_or_default();
        // This string is shortened.
        assert_eq!("It helps", output);
    }

    #[test]
    fn test_file_filter() {
        let args = HashMap::new();
        //
        //
        // Test file stem.
        let input =
            "/usr/local/WEB-SERVER-CONTENT/blog.getreu.net/projects/tp-note/20200908-My file.md";
        let output = file_stem_filter(&to_value(input).unwrap(), &args).unwrap_or_default();
        assert_eq!("My file", output);

        let input =
            "/usr/local/WEB-SERVER-CONTENT/blog.getreu.net/projects/tp-note/20200908-My dir/";
        let output = file_stem_filter(&to_value(input).unwrap(), &args).unwrap_or_default();
        assert_eq!("My dir", output);
        //
        //
        // Test file tag.
        let input =
            "/usr/local/WEB-SERVER-CONTENT/blog.getreu.net/projects/tp-note/20200908-My file.md";
        let output = file_sort_tag_filter(&to_value(input).unwrap(), &args).unwrap_or_default();
        assert_eq!("20200908", output);

        let input =
            "/usr/local/WEB-SERVER-CONTENT/blog.getreu.net/projects/tp-note/20200908-My dir/";
        let output = file_sort_tag_filter(&to_value(input).unwrap(), &args).unwrap_or_default();
        assert_eq!("20200908", output);
        //
        //
        // Test file extension.
        let input =
            "/usr/local/WEB-SERVER-CONTENT/blog.getreu.net/projects/tp-note/20200908-My file.md";
        let output = file_ext_filter(&to_value(input).unwrap(), &args).unwrap_or_default();
        assert_eq!("md", output);

        let input =
            "/usr/local/WEB-SERVER-CONTENT/blog.getreu.net/projects/tp-note/20200908-My file.pfd.md";
        let output = file_ext_filter(&to_value(input).unwrap(), &args).unwrap_or_default();
        assert_eq!("md", output);

        let input =
            "/usr/local/WEB-SERVER-CONTENT/blog.getreu.net/projects/tp-note/20200908-My dir/";
        let output = file_ext_filter(&to_value(input).unwrap(), &args).unwrap_or_default();
        assert_eq!("", output);
        //
        //
        // Test copy counter filter.
        let input =
            "/usr/local/WEB-SERVER-CONTENT/blog.getreu.net/projects/tp-note/20200908-My file(123).md";
        let output = file_copy_counter_filter(&to_value(input).unwrap(), &args).unwrap_or_default();
        assert_eq!(123, output);

        let input =
            "/usr/local/WEB-SERVER-CONTENT/blog.getreu.net/projects/tp-note/20200908-My dir/";
        let output = file_ext_filter(&to_value(input).unwrap(), &args).unwrap_or_default();
        assert_eq!("", output);
        //
        //
        // Test filename .
        let input =
            "/usr/local/WEB-SERVER-CONTENT/blog.getreu.net/projects/tp-note/20200908-My file(123).md";
        let output = file_name_filter(&to_value(input).unwrap(), &args).unwrap_or_default();
        assert_eq!("20200908-My file(123).md", output);

        let input =
            "/usr/local/WEB-SERVER-CONTENT/blog.getreu.net/projects/tp-note/20200908-My dir/";
        let output = file_ext_filter(&to_value(input).unwrap(), &args).unwrap_or_default();
        assert_eq!("", output);
        //
        //
        // Test `prepend_dot`.
        let mut args = HashMap::new();
        args.insert("with".to_string(), to_value(".").unwrap());
        let input = "md";
        let output = prepend_filter(&to_value(input).unwrap(), &args).unwrap_or_default();
        assert_eq!(".md", output);

        let input = "";
        let output = prepend_filter(&to_value(input).unwrap(), &args).unwrap_or_default();
        assert_eq!("", output);
    }

    #[test]
    fn test_get_lang_filter() {
        //
        // Test `get_lang_filter()`
        use crate::settings::Settings;
        use lingua::IsoCode639_1;

        // The `get_lang` filter requires an initialized `SETTINGS` object.
        // Lock the config object for this test.
        let filter_get_lang = FilterGetLang::SomeLanguages(vec![
            IsoCode639_1::DE,
            IsoCode639_1::EN,
            IsoCode639_1::FR,
        ]);

        let mut settings = SETTINGS.write();
        *settings = Settings::default();
        settings.filter_get_lang = filter_get_lang;
        // This locks `SETTINGS` for further write access in this scope.
        let _settings = RwLockWriteGuard::<'_, _>::downgrade(settings);

        let args = HashMap::new();
        let input = "Das große Haus";
        let output = get_lang_filter(&to_value(input).unwrap(), &args).unwrap_or_default();
        assert_eq!("de", output);

        let args = HashMap::new();
        let input = "Il est venu trop tard";
        let output = get_lang_filter(&to_value(input).unwrap(), &args).unwrap_or_default();
        assert_eq!("fr", output);

        let args = HashMap::new();
        let input = "How to set up a roof rack";
        let output = get_lang_filter(&to_value(input).unwrap(), &args).unwrap_or_default();
        assert_eq!("en", output);

        let args = HashMap::new();
        let input = "1917039480 50198%-328470";
        let output = get_lang_filter(&to_value(input).unwrap(), &args).unwrap_or_default();
        assert_eq!("", output);

        let args = HashMap::new();
        let input = " \t\n ";
        let output = get_lang_filter(&to_value(input).unwrap(), &args).unwrap_or_default();
        assert_eq!("", output);
        // Release the lock.
        drop(_settings);
    }

    #[test]
    fn test_map_lang_filter() {
        //
        // `Test `map_lang_filter()`
        use crate::settings::Settings;

        let mut filter_map_lang_btmap = BTreeMap::new();
        filter_map_lang_btmap.insert("de".to_string(), "de-DE".to_string());
        let mut settings = SETTINGS.write();
        *settings = Settings::default();
        settings.filter_map_lang_btmap = Some(filter_map_lang_btmap);

        // This locks `SETTINGS` for further write access in this scope.
        let _settings = RwLockWriteGuard::<'_, _>::downgrade(settings);

        let args = HashMap::new();
        let input = "de";
        let output = map_lang_filter(&to_value(input).unwrap(), &args).unwrap_or_default();
        assert_eq!("de-DE", output);

        let args = HashMap::new();
        let input = "xyz";
        let output = map_lang_filter(&to_value(input).unwrap(), &args).unwrap_or_default();
        assert_eq!("xyz", output);

        let args = HashMap::new();
        let input = " \t\n ";
        let output = map_lang_filter(&to_value(input).unwrap(), &args).unwrap_or_default();
        assert_eq!(to_value("").unwrap(), output);

        let mut args = HashMap::new();
        args.insert("default".to_string(), to_value("test").unwrap());
        let input = " \t\n ";
        let output = map_lang_filter(&to_value(input).unwrap(), &args).unwrap_or_default();
        assert_eq!("test".to_string(), output);

        drop(_settings);
    }

    #[test]
    fn test_parse_hyperlink() {
        use super::Hyperlink;
        // Stand alone Markdown link.
        let input = r#"abc[Homepage](https://blog.getreu.net "My blog")abc"#;
        let expected_output = Hyperlink {
            name: "Homepage".to_string(),
            target: "https://blog.getreu.net".to_string(),
            title: "My blog".to_string(),
        };
        let output = Hyperlink::from(input);
        assert_eq!(expected_output, output.unwrap());

        // Markdown link reference.
        let input = r#"abc[Homepage][home]abc
                      [home]: https://blog.getreu.net "My blog""#;
        let expected_output = Hyperlink {
            name: "Homepage".to_string(),
            target: "https://blog.getreu.net".to_string(),
            title: "My blog".to_string(),
        };
        let output = Hyperlink::from(input);
        assert_eq!(expected_output, output.unwrap());

        //
        // RestructuredText link
        let input = "abc`Homepage <https://blog.getreu.net>`_\nabc";
        let expected_output = Hyperlink {
            name: "Homepage".to_string(),
            target: "https://blog.getreu.net".to_string(),
            title: "".to_string(),
        };
        let output = Hyperlink::from(input);
        assert_eq!(expected_output, output.unwrap());

        //
        // RestructuredText link ref
        let input = "abc `Homepage<home_>`_ abc\n.. _home: https://blog.getreu.net\nabc";
        let expected_output = Hyperlink {
            name: "Homepage".to_string(),
            target: "https://blog.getreu.net".to_string(),
            title: "".to_string(),
        };
        let output = Hyperlink::from(input);
        assert_eq!(expected_output, output.unwrap());
    }
}
