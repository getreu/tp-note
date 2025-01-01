//! Extends the built-in Tera filters.
use crate::config::Scheme;
use crate::config::FILENAME_DOTFILE_MARKER;
use crate::config::LIB_CFG;
use crate::config::TMPL_VAR_FM_;
use crate::filename::NotePath;
use crate::filename::NotePathBuf;
use crate::filename::NotePathStr;
use crate::markup_language::InputConverter;
use crate::markup_language::MarkupLanguage;
#[cfg(feature = "lang-detection")]
use crate::settings::FilterGetLang;
use crate::settings::SETTINGS;
#[cfg(feature = "lang-detection")]
use lingua::{LanguageDetector, LanguageDetectorBuilder};
use parse_hyperlinks::iterator::MarkupLink;
use parse_hyperlinks::parser::Link;
use sanitize_filename_reader_friendly::sanitize;
use std::borrow::Cow;
use std::collections::HashMap;
use std::hash::BuildHasher;
use std::path::Path;
use std::path::PathBuf;
use std::sync::LazyLock;
use tera::Map;
use tera::{try_get_value, Result as TeraResult, Tera, Value};

/// Filter parameter of the `cut_filter()` limiting the maximum length of
/// template variables. The filter is usually used to in the note's front matter
/// as title. For example: the title should not be too long, because it will end
/// up as part of the filename when the note is saved to disk. Filenames of some
/// operating systems are limited to 255 bytes.
#[cfg(not(test))]
const CUT_LEN_MAX: usize = 200;
#[cfg(test)]
pub const CUT_LEN_MAX: usize = 10;

/// Lowercase pattern to detect HTML in stdin.
const HTML_PAT1: &str = "<!doctype html";

/// Lowercase pattern to detect HTML in stdin.
const HTML_PAT2: &str = "<html";

/// Tera object with custom functions registered.
pub static TERA: LazyLock<Tera> = LazyLock::new(|| {
    let mut tera = Tera::default();
    tera.register_filter("append", append_filter);
    tera.register_filter("cut", cut_filter);
    tera.register_filter("file_copy_counter", file_copy_counter_filter);
    tera.register_filter("file_ext", file_ext_filter);
    tera.register_filter("file_name", file_name_filter);
    tera.register_filter("file_sort_tag", file_sort_tag_filter);
    tera.register_filter("file_stem", file_stem_filter);
    tera.register_filter("file_copy_counter", file_copy_counter_filter);
    tera.register_filter("file_name", file_name_filter);
    tera.register_filter("file_ext", file_ext_filter);
    tera.register_filter("find_last_created_file", find_last_created_file);
    tera.register_filter("html_to_markup", html_to_markup_filter);
    tera.register_filter("incr_sort_tag", incr_sort_tag_filter);
    tera.register_filter("prepend", prepend_filter);
    tera.register_filter("append", append_filter);
    tera.register_filter("remove", remove_filter);
    tera.register_filter("insert", insert_filter);
    tera.register_filter("get_lang", get_lang_filter);
    tera.register_filter("heading", heading_filter);
    tera.register_filter("insert", insert_filter);
    tera.register_filter("link_dest", link_dest_filter);
    tera.register_filter("link_text", link_text_filter);
    tera.register_filter("link_text_picky", link_text_picky_filter);
    tera.register_filter("link_title", link_title_filter);
    tera.register_filter("html_heading", html_heading_filter);
    tera.register_filter("map_lang", map_lang_filter);
    tera.register_filter("markup_to_html", markup_to_html_filter);
    tera.register_filter("name", name_filter);
    tera.register_filter("prepend", prepend_filter);
    tera.register_filter("replace_empty", replace_empty_filter);
    tera.register_filter("remove", remove_filter);
    tera.register_filter("sanit", sanit_filter);
    tera.register_filter("to_html", to_html_filter);
    tera.register_filter("to_yaml", to_yaml_filter);
    tera.register_filter("trim_file_sort_tag", trim_file_sort_tag_filter);
    tera
});

/// A filter converting any input `tera::Value` into a `tera::Value::String(s)`
/// with `s` being the YAML representation of the object. The input can be of
/// any type, the output type is alwasy a `Value::String()`.
/// If the input type is `tera::Value::Object`, all top level keys starting with
/// `fm_` are  localized (see `fm_var.localization`).
/// When the optional parameter `key='k'` is given, the input is regarded as
/// the corresponding value to this key.
/// The optional parameter `tab=n` indents the YAML values `n` characters to
/// the right of the first character of the key by inserting additional spaces
/// between the key and the value. When `tab=n` is given, it has precedence
/// over the  default value, read from the configuration file variable
/// `tmpl.filter.to_yaml_tab`.
fn to_yaml_filter<S: BuildHasher>(
    val: &Value,
    args: &HashMap<String, Value, S>,
) -> TeraResult<Value> {
    let scheme = &LIB_CFG.read_recursive().scheme[SETTINGS.read_recursive().current_scheme];

    let val_yaml = if let Some(Value::String(k)) = args.get("key") {
        let mut m = tera::Map::new();
        let k = name(scheme, k);
        m.insert(k.to_owned(), val.to_owned());
        serde_yaml::to_string(&m).unwrap()
    } else {
        match &val {
            Value::Object(map) => {
                let mut m = Map::new();
                for (k, v) in map.into_iter() {
                    //
                    let new_k = name(scheme, k);
                    m.insert(new_k.to_owned(), v.to_owned());
                }
                let o = serde_json::Value::Object(m);
                serde_yaml::to_string(&o).unwrap()
            }
            &oo => serde_yaml::to_string(oo).unwrap(),
        }
    };

    // Translate the empty set, into an empty string and return it.
    if val_yaml.trim_end() == "{}" {
        return Ok(tera::Value::String("".to_string()));
    }

    // Formatting: adjust indent.
    let val_yaml: String = if let Some(tab) =
        args.get("tab").and_then(|v| v.as_u64()).or_else(|| {
            let n = scheme.tmpl.filter.to_yaml_tab;
            if n == 0 {
                None
            } else {
                Some(n)
            }
        }) {
        val_yaml
            .lines()
            .map(|l| {
                let mut insert_pos = 0;
                let mut inserts_n = 0;
                if let Some(colpos) = l.find(": ") {
                    if let Some(key_pos) = l.find(char::is_alphabetic) {
                        if key_pos < colpos
                            && !l.find('\'').is_some_and(|p| p < colpos)
                            && !l.find("\"'").is_some_and(|p| p < colpos)
                        {
                            insert_pos = colpos + ": ".len();
                            inserts_n = (tab as usize).saturating_sub(insert_pos);
                        }
                    }
                } else if l.starts_with("- ") {
                    inserts_n = tab as usize;
                };

                // Enlarge indent.
                let mut l = l.to_owned();
                let strut = " ".repeat(inserts_n);
                // If `insert>0`, we know that `colon_pos>0`.
                // `colon_pos+1` inserts between `: `.
                l.insert_str(insert_pos, &strut);
                l.push('\n');
                l
            })
            .collect::<String>()
    } else {
        val_yaml
    };

    let val_yaml = val_yaml.trim_end().to_owned();

    Ok(Value::String(val_yaml))
}

/// A filter that coverts a `tera::Value` tree into an HTML representation,
/// with following HTLM tags:
/// * `Value::Object`: `<blockquote class="fm">` and `<div class="fm">`,
/// * `Value::Array`: `<ul class="fm">` and `<li class="fm">`,
/// * `Value::String`: no tag,
/// * Other non-string basic types: `<code class="fm">`.
///
/// The input can be of any type, the output type is `Value::String()`.
/// If the input type is `Value::Object`, all top level keys starting with
/// `fm_` are  localized (see `fm_var.localization`).
/// Note: HTML templates escape HTML critical characters by default.
/// To use the `to_hmtl` filter in HTML templates, add a `safe` filter in last
/// position. This is no risk, as the `to_html` filter always escapes string
/// values automatically, regardless of the template type.
fn to_html_filter<S: BuildHasher>(
    value: &Value,
    _args: &HashMap<String, Value, S>,
) -> TeraResult<Value> {
    fn tag_to_html(val: Value, is_root: bool, output: &mut String) {
        match val {
            Value::Array(a) => {
                output.push_str("<ul class=\"fm\">");
                for i in a {
                    output.push_str("<li class=\"fm\">");
                    tag_to_html(i, false, output);
                    output.push_str("</li>");
                }
                output.push_str("</ul>");
            }

            Value::String(s) => output.push_str(&html_escape::encode_text(&s)),

            Value::Object(map) => {
                output.push_str("<blockquote class=\"fm\">");
                if is_root {
                    let scheme =
                        &LIB_CFG.read_recursive().scheme[SETTINGS.read_recursive().current_scheme];
                    for (k, v) in map {
                        output.push_str("<div class=\"fm\">");
                        output.push_str(name(scheme, &k));
                        output.push_str(": ");
                        tag_to_html(v, false, output);
                        output.push_str("</div>");
                    }
                } else {
                    for (k, v) in map {
                        output.push_str("<div class=\"fm\">");
                        output.push_str(&k);
                        output.push_str(": ");
                        tag_to_html(v, false, output);
                        output.push_str("</div>");
                    }
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

    let mut html = String::new();
    tag_to_html(value.to_owned(), true, &mut html);

    Ok(Value::String(html))
}

/// This filter translates `fm_*` header variable names into some human
/// language. Suppose we have:
/// ```rust, ignore
/// scheme.tmpl.variables.names_assertions = []
/// `[ "fm_lang", "Sprache", [], ],
/// ]
/// ```
/// Then, the expression `'fm_lang'|name` resolves into `Sprache`.
/// For variables not listed below, only the prefix `fm_` is stripped and
/// no translation occurs, e.g. `'fm_unknown'|name` becomes `unknown`.
/// The input type must be `Value::String` and the output type is
/// `Value::String`.
fn name_filter<S: BuildHasher>(
    value: &Value,
    _args: &HashMap<String, Value, S>,
) -> TeraResult<Value> {
    let input = try_get_value!("translate", "value", String, value);

    // This replaces the `fm`-name in the key by the localized name.
    let scheme = &LIB_CFG.read_recursive().scheme[SETTINGS.read_recursive().current_scheme];
    let output = name(scheme, &input);
    Ok(Value::String(output.to_string()))
}

/// Returns the localized header field name. For example: `fm_subtitle`
/// resolves into `Untertitel`. The configuration file variable
/// '`scheme.tmpl.variables.names_assertions`' contains the translation table.
pub(crate) fn name<'a>(scheme: &'a Scheme, input: &'a str) -> &'a str {
    let vars = &scheme.tmpl.fm_var.localization;
    vars.iter().find(|&l| l.0 == input).map_or_else(
        || input.strip_prefix(TMPL_VAR_FM_).unwrap_or(input),
        |l| &l.1,
    )
}

/// A filter that converts incoming HTML into some target markup language.
/// The parameter file `extension` indicates in what Markup
/// language the input is written. When no `extension` is given, the filler
/// does not convert, it just passes through.
/// This filter only converts, if the first line of the input stream starts with
/// the pattern `<html` or `<!DOCTYPE html`.
/// In any case, the output of the converter is trimmed at the end
/// (`trim_end()`).
fn html_to_markup_filter<S: BuildHasher>(
    value: &Value,
    #[allow(unused_variables)] args: &HashMap<String, Value, S>,
) -> TeraResult<Value> {
    #[allow(unused_mut)]
    let mut buffer = try_get_value!("html_to_markup", "value", String, value);

    let default = if let Some(default_val) = args.get("default") {
        try_get_value!("markup_to_html", "default", String, default_val)
    } else {
        String::new()
    };

    let firstline = buffer
        .lines()
        .next()
        .map(|l| l.trim_start().to_ascii_lowercase());
    if firstline.is_some_and(|l| l.starts_with(HTML_PAT1) || l.starts_with(HTML_PAT2)) {
        let extension = if let Some(ext) = args.get("extension") {
            try_get_value!("markup_to_html", "extension", String, ext)
        } else {
            String::new()
        };

        let converter = InputConverter::build(&extension);
        buffer = match converter(buffer) {
            Ok(converted) if converted.is_empty() => default,
            Ok(converted) => converted,
            Err(e) => {
                log::info!("{}", e.to_string());
                default
            }
        };
    } else {
        buffer = default;
    }

    // Trim end without reallocation.
    buffer.truncate(buffer.trim_end().len());

    Ok(Value::String(buffer))
}

/// Takes the markup formatted input and renders it to HTML.
/// The parameter file `extension` indicates in what Markup
/// language the input is written.
/// When `extension` is not given or known, the renderer defaults to
/// `MarkupLanguage::Unknown`.
/// The input types must be `Value::String` and the output type is
/// `Value::String()`
fn markup_to_html_filter<S: BuildHasher>(
    value: &Value,
    args: &HashMap<String, Value, S>,
) -> TeraResult<Value> {
    let input = try_get_value!("markup_to_html", "value", String, value);

    let markup_language = if let Some(ext) = args.get("extension") {
        let ext = try_get_value!("markup_to_html", "extension", String, ext);
        let ml = MarkupLanguage::from(ext.as_str());
        if ml.is_some() {
            ml
        } else {
            MarkupLanguage::Unkown
        }
    } else {
        MarkupLanguage::Unkown
    };

    // Render the markup language.
    let html_output = markup_language.render(&input);

    Ok(Value::String(html_output))
}

/// Adds a new filter to Tera templates:
/// `sanit` or `sanit()` sanitizes a string so that it can be used to
/// assemble filenames or paths. In addition, `sanit(alpha=true)` prepends
/// the `sort_tag.extra_separator` when the result starts with one of
/// `sort_tag.extra_chars`, usually a number. This way we guaranty that the filename
/// never starts with a number. We do not allow this, to be able to distinguish
/// reliably the sort tag from the filename. In addition to the above, the
/// filter checks if the string represents a "well-formed" filename. If it
/// is the case, and the filename starts with a dot, the file is prepended by
/// `sort_tag.extra_separator`.
/// The input type must be `Value::String` and the output type is
/// `Value::String()`
fn sanit_filter<S: BuildHasher>(
    value: &Value,
    _args: &HashMap<String, Value, S>,
) -> TeraResult<Value> {
    let input = try_get_value!("sanit", "value", String, value);

    // Check if this is a usual dotfile filename.
    let is_dotfile = input.starts_with(FILENAME_DOTFILE_MARKER)
        && PathBuf::from(&*input).has_wellformed_filename();

    // Sanitize string.
    let mut res = sanitize(&input);

    // If `FILNAME_DOTFILE_MARKER` was stripped, prepend one.
    if is_dotfile && !res.starts_with(FILENAME_DOTFILE_MARKER) {
        res.insert(0, FILENAME_DOTFILE_MARKER);
    }

    Ok(Value::String(res))
}

/// A Tera filter that searches for the first Markdown or reStructuredText link
/// in the input stream and returns the link's name (link text).
/// If not found, it returns the empty string.
/// The input type must be `Value::String` and the output type is
/// `Value::String()`
fn link_text_filter<S: BuildHasher>(
    value: &Value,
    _args: &HashMap<String, Value, S>,
) -> TeraResult<Value> {
    let input = try_get_value!("link_text", "value", String, value);

    let hyperlink = FirstHyperlink::from(&input).unwrap_or_default();

    Ok(Value::String(hyperlink.text.to_string()))
}

/// A Tera filter that searches for the first Markdown or reStructuredText link
/// in the input stream and returns the link's URL.
/// If not found, it returns the empty string.
/// The input type must be `Value::String` and the output type is
/// `Value::String()`
fn link_dest_filter<S: BuildHasher>(
    value: &Value,
    _args: &HashMap<String, Value, S>,
) -> TeraResult<Value> {
    let p = try_get_value!("link_dest", "value", String, value);

    let hyperlink = FirstHyperlink::from(&p).unwrap_or_default();

    Ok(Value::String(hyperlink.dest.to_string()))
}

/// A Tera filter that searches for the first Markdown or reStructuredText link
/// in the input stream and returns the link's text's name (link text).
/// Unlike the filter `link_dest`, it does not necessarily return the first
/// finding. For example, it skips autolinks, local links and links
/// with some URL in the link text.
/// If not found, it returns the empty string.
/// The input type must be `Value::String` and the output type is
/// `Value::String()`
fn link_text_picky_filter<S: BuildHasher>(
    value: &Value,
    _args: &HashMap<String, Value, S>,
) -> TeraResult<Value> {
    let p = try_get_value!("link_text_picky", "value", String, value);

    let hyperlink = FirstHyperlink::from_picky(&p).unwrap_or_default();

    Ok(Value::String(hyperlink.text.to_string()))
}

/// A Tera filter that searches for the first Markdown or reStructuredText link
/// in the input stream and returns the link's title.
/// If not found, it returns the empty string.
/// The input type must be `Value::String` and the output type is
/// `Value::String()`
fn link_title_filter<S: BuildHasher>(
    value: &Value,
    _args: &HashMap<String, Value, S>,
) -> TeraResult<Value> {
    let p = try_get_value!("link_title", "value", String, value);

    let hyperlink = FirstHyperlink::from(&p).unwrap_or_default();

    Ok(Value::String(hyperlink.title.to_string()))
}

/// A Tera filter that searches for the first HTML heading
/// in the HTML input stream and returns the heading text.
/// If not found, it returns the empty string.
/// The input type must be `Value::String` and the output type is
/// `Value::String()`
fn html_heading_filter<S: BuildHasher>(
    value: &Value,
    _args: &HashMap<String, Value, S>,
) -> TeraResult<Value> {
    let p = try_get_value!("html_heading", "value", String, value);

    let html_heading = FirstHtmlHeading::from(&p).unwrap_or_default();

    Ok(Value::String(html_heading.0.to_string()))
}

/// A Tera filter that truncates the input stream and returns the
/// max `CUT_LEN_MAX` bytes of valid UTF-8.
/// The input type must be `Value::String` and the output type is
/// `Value::String()`
fn cut_filter<S: BuildHasher>(
    value: &Value,
    _args: &HashMap<String, Value, S>,
) -> TeraResult<Value> {
    let input = try_get_value!("cut", "value", String, value);

    let mut short = "";
    for i in (0..CUT_LEN_MAX).rev() {
        if let Some(s) = input.get(..i) {
            short = s;
            break;
        }
    }
    Ok(Value::String(short.to_owned()))
}

/// A Tera filter that returns the first line or the first sentence of the input
/// stream.
/// The input type must be `Value::String` and the output type is
/// `Value::String()`
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

    Ok(Value::String(content_heading))
}

/// A Tera filter that takes a path and extracts the tag of the filename.
/// The input type must be `Value::String` and the output type is
/// `Value::String()`
fn file_sort_tag_filter<S: BuildHasher>(
    value: &Value,
    _args: &HashMap<String, Value, S>,
) -> TeraResult<Value> {
    let p = try_get_value!("file_sort_tag", "value", String, value);
    let p = PathBuf::from(p);
    let (tag, _, _, _, _) = p.disassemble();

    Ok(Value::String(tag.to_owned()))
}

/// A Tera filter that takes a path and extracts its last element.
/// This function trims the `sort_tag` if present.
/// The input type must be `Value::String` and the output type is
/// `Value::String()`
fn trim_file_sort_tag_filter<S: BuildHasher>(
    value: &Value,
    _args: &HashMap<String, Value, S>,
) -> TeraResult<Value> {
    let input = try_get_value!("trim_file_sort_tag", "value", String, value);
    let input = PathBuf::from(input);
    let (_, fname, _, _, _) = input.disassemble();

    Ok(Value::String(fname.to_owned()))
}

/// A Tera filter that takes a path and extracts its file stem,
/// in other words: the filename without `sort_tag`, `file_copy_counter`
/// and `extension`.
/// The input type must be `Value::String` and the output type is
/// `Value::String()`
fn file_stem_filter<S: BuildHasher>(
    value: &Value,
    _args: &HashMap<String, Value, S>,
) -> TeraResult<Value> {
    let input = try_get_value!("file_stem", "value", String, value);
    let input = PathBuf::from(input);
    let (_, _, stem, _, _) = input.disassemble();

    Ok(Value::String(stem.to_owned()))
}

/// A Tera filter that takes a path and extracts its copy counter,
/// or, to put it another way: the filename without `sort_tag`, `file_stem`
/// and `file_ext` (and their separators). If the filename contains a
/// `copy_counter=n`, the returned JSON value variant is `Value::Number(n)`.
/// If there is no copy counter in the input, the output is `Value::Number(0)`.
/// The input type must be `Value::String` and the output type is
/// `Value::Number()`
fn file_copy_counter_filter<S: BuildHasher>(
    value: &Value,
    _args: &HashMap<String, Value, S>,
) -> TeraResult<Value> {
    let input = try_get_value!("file_copy_counter", "value", String, value);
    let input = PathBuf::from(input);
    let (_, _, _, copy_counter, _) = input.disassemble();
    let copy_counter = copy_counter.unwrap_or(0);

    Ok(Value::from(copy_counter))
}

/// A Tera filter that takes a path and extracts its filename without
/// file extension. The filename may contain a sort-tag, a copy-counter and
/// also separators.
/// The input type must be `Value::String` and the output type is
/// `Value::String()`
fn file_name_filter<S: BuildHasher>(
    value: &Value,
    _args: &HashMap<String, Value, S>,
) -> TeraResult<Value> {
    let p = try_get_value!("file_name", "value", String, value);

    let filename = Path::new(&p)
        .file_name()
        .unwrap_or_default()
        .to_str()
        .unwrap_or_default()
        .to_owned();

    Ok(Value::String(filename))
}

/// A Tera filter that replace the input string with the parameter `with`, but
/// only if the input stream is empty.
/// The input type and the type of the parameter `with`
/// must be a `Value::String`.
fn replace_empty_filter<S: BuildHasher>(
    value: &Value,
    args: &HashMap<String, Value, S>,
) -> TeraResult<Value> {
    let input = try_get_value!("replace_empty", "value", String, value);

    let mut res = input;

    if let Some(with) = args.get("with") {
        let with = try_get_value!("replace_empty", "with", String, with);
        if res.is_empty() {
            res = with;
        };
    }
    Ok(Value::String(res))
}

/// A Tera filter that prepends the string parameter `with`, but only if the
/// input stream is not empty.
/// In addition, the flag `newline` inserts a newline character at end of the
/// result. In case the input stream is empty nothing is appended.
/// When called with the strings parameter `with_sort_tag`, the filter
/// prepends the sort-tag and all necessary sort-tag separator characters,
/// regardless whether the input stream in empty or not.
/// The input type, and the type of the parameter `with` and   `with_sort_tag`
/// must be `Value::String`. The parameter `newline` must be a `Value::Bool` and
/// the output type is `Value::String()`.
fn prepend_filter<S: BuildHasher>(
    value: &Value,
    args: &HashMap<String, Value, S>,
) -> TeraResult<Value> {
    let input = try_get_value!("prepend", "value", String, value);

    let mut res = input;

    if let Some(with) = args.get("with") {
        let with = try_get_value!("prepend", "with", String, with);
        let mut s = String::new();
        if !res.is_empty() {
            s.push_str(&with);
            s.push_str(&res);
            res = s;
        };
    } else if let Some(sort_tag) = args.get("with_sort_tag") {
        let sort_tag = try_get_value!("prepend", "with_sort_tag", String, sort_tag);
        res = PathBuf::from_disassembled(&sort_tag, &res, None, "")
            .to_str()
            .unwrap_or_default()
            .to_string();
    };

    if let Some(Value::Bool(newline)) = args.get("newline") {
        if *newline && !res.is_empty() {
            let mut s = String::new();
            s.push('\n');
            s.push_str(&res);
            res = s;
        }
    };

    Ok(Value::String(res))
}

/// A Tera filter that appends the string parameter `with`. In addition, the
/// flag `newline` inserts a newline character at end of the result. In
/// case the input stream is empty,  nothing is appended.
/// The input type, and the type of the parameter `with`  must be
/// `Value::String`. The parameter `newline` must be a `Value::Bool` and the
/// output type is `Value::String()`.
fn append_filter<S: BuildHasher>(
    value: &Value,
    args: &HashMap<String, Value, S>,
) -> TeraResult<Value> {
    let input = try_get_value!("append", "value", String, value);

    if input.is_empty() {
        return Ok(Value::String("".to_string()));
    }

    let mut res = input.clone();
    if let Some(with) = args.get("with") {
        let with = try_get_value!("append", "with", String, with);
        res.push_str(&with);
    };

    if let Some(newline) = args.get("newline") {
        let newline = try_get_value!("newline", "newline", bool, newline);
        if newline && !res.is_empty() {
            res.push('\n');
        }
    };

    Ok(Value::String(res))
}

/// A Tera filter that takes a path and extracts its file extension.
/// The input type must be `Value::String()`, the output type is
/// `Value::String()`.
fn file_ext_filter<S: BuildHasher>(
    value: &Value,
    _args: &HashMap<String, Value, S>,
) -> TeraResult<Value> {
    let p = try_get_value!("file_ext", "value", String, value);

    let ext = Path::new(&p)
        .extension()
        .unwrap_or_default()
        .to_str()
        .unwrap_or_default()
        .to_owned();

    Ok(Value::String(ext))
}

/// A Tera filter that takes a directory path and returns the alphabetically
/// last sort-tag of all Tp-Note documents in that directory.
/// The filter returns the empty string if none was found.
/// The input type must be `Value::String()`, the output type is
/// `Value::String()`.
fn find_last_created_file<S: BuildHasher>(
    value: &Value,
    _args: &HashMap<String, Value, S>,
) -> TeraResult<Value> {
    let p_str = try_get_value!("dir_last_created", "value", String, value);

    let p = Path::new(&p_str);
    let last = match p.find_last_created_file() {
        Some(filename) => Path::join(p, Path::new(&filename))
            .to_str()
            .unwrap()
            .to_string(),
        None => String::new(),
    };

    Ok(Value::String(last.to_string()))
}

/// Expects a path a filename in its input and returns an incremented sequential
/// sort-tag.
/// First, from the input's filename the sort-tag is extracted. Then, it
/// matches all digits from the end of the sort- tag, increments them
/// and replaces the matched digits with the result. If no numeric digits can be
/// matched, consider alphabetic letters as base 26 number system and try again.
/// Returns the default value if no match succeeds.
/// Note, that only sequential sort-tags are incremented, for others or, if the
/// input is empty, `default` is returned.
/// The path in the input allows to check if the resulting sort-tag exists
/// on disk already. If this is the case, a subcategory is appended to the
/// resulting sort-tag.
/// All input types are `Value::String`. The output type is `Value::String()`.
fn incr_sort_tag_filter<S: BuildHasher>(
    value: &Value,
    args: &HashMap<String, Value, S>,
) -> TeraResult<Value> {
    let input = try_get_value!("incr_sort_tag", "value", String, value);

    let mut default = String::new();
    if let Some(d) = args.get("default") {
        default = try_get_value!("incr_sort_tag", "default", String, d);
    };

    let (input_dir, filename) = input.rsplit_once(['/', '\\']).unwrap_or(("", &input));
    let (input_sort_tag, _, is_sequential) = filename.split_sort_tag(false);

    if input_sort_tag.is_empty() || !is_sequential {
        return Ok(Value::String(default));
    }

    // Start analysing the input.
    let (prefix, digits) = match input_sort_tag.rfind(|c: char| !c.is_ascii_digit()) {
        Some(idx) => (&input_sort_tag[..idx + 1], &input_sort_tag[idx + 1..]),
        None => ("", input_sort_tag),
    };

    // Search for digits
    let mut output_sort_tag = if !digits.is_empty() {
        // Return early if this number is too big.
        const DIGITS_MAX: usize = u32::MAX.ilog10() as usize; // 9
        if digits.len() > DIGITS_MAX {
            return Ok(Value::String(default));
        }

        // Convert string to n base 10.
        let mut n = match digits.parse::<u32>() {
            Ok(n) => n,
            _ => return Ok(Value::String(default)),
        };

        n += 1;

        let mut res = n.to_string();
        if res.len() < digits.len() {
            let padding = "0".repeat(digits.len() - res.len());
            res = format!("{}{}", padding, res);
        }

        // Assemble sort-tag.
        prefix.to_string() + &res
    } else {
        //
        // Search for letters as digits
        let (prefix, letters) = match input_sort_tag.rfind(|c: char| !c.is_ascii_lowercase()) {
            Some(idx) => (&input_sort_tag[..idx + 1], &input_sort_tag[idx + 1..]),
            None => ("", input_sort_tag),
        };

        if !letters.is_empty() {
            const LETTERS_BASE: u32 = 26;
            const LETTERS_MAX: usize = (u32::MAX.ilog2() / (LETTERS_BASE.ilog2() + 1)) as usize; // 6=31/(4+1)

            // Return early if this number is too big.
            if letters.len() > LETTERS_MAX {
                return Ok(Value::String(default));
            }

            // Interpret letters as base LETTERS_BASE and convert to int.
            let mut n = letters.chars().fold(0, |acc, c| {
                LETTERS_BASE * acc + (c as u8).saturating_sub(b'a') as u32
            });

            n += 1;

            // Convert back to letters base LETTERS_BASE.
            let mut res = String::new();
            while n > 0 {
                let c = char::from_u32('a' as u32 + n.rem_euclid(LETTERS_BASE)).unwrap_or_default();
                n = n.div_euclid(LETTERS_BASE);
                res = format!("{}{}", c, res);
            }
            if res.len() < letters.len() {
                let padding = "a".repeat(letters.len() - res.len());
                res = format!("{}{}", padding, res);
            }

            // Assemble sort-tag.
            prefix.to_string() + &res
        } else {
            default
        }
    };

    // Check for a free slot, branch if not free.
    let input_dir = Path::new(input_dir);
    if input_dir.has_file_with_sort_tag(&output_sort_tag).is_some() {
        output_sort_tag = input_sort_tag.to_string();
    }
    while input_dir.has_file_with_sort_tag(&output_sort_tag).is_some() {
        if output_sort_tag
            .chars()
            .last()
            .is_some_and(|c| c.is_ascii_digit())
        {
            output_sort_tag.push('a')
        } else {
            output_sort_tag.push('1')
        }
    }

    Ok(Value::String(output_sort_tag))
}

/// A Tera filter that takes a map of variables/values and removes a key/value
/// pair with the parameter `remove(key="<var-name>").
/// The input type must be `Value::Object()`, the parameter must be
/// `Value::String()` and the output type is `Value::Object()`.
fn remove_filter<S: BuildHasher>(
    value: &Value,
    args: &HashMap<String, Value, S>,
) -> TeraResult<Value> {
    let mut map = try_get_value!("remove", "value", tera::Map<String, tera::Value>, value);

    if let Some(outkey) = args.get("key") {
        let outkey = try_get_value!("remove", "key", String, outkey);
        let _ = map.remove(&outkey);
    };

    Ok(Value::Object(map))
}

/// A Tera filter that takes a map of key/values and inserts a key/value pair
/// with the parameters `insert(key="<var-name>", value=<var-value>). If the
/// variable exists in the map already, its value is replaced.
/// The input type must be `Value::Object()`, the `key` parameter must be a
/// `Value::String()` and the output type is `Value::Object()`.
fn insert_filter<S: BuildHasher>(
    value: &Value,
    args: &HashMap<String, Value, S>,
) -> TeraResult<Value> {
    let mut map = try_get_value!("insert", "value", tera::Map<String, tera::Value>, value);

    if let Some(inkey) = args.get("key") {
        let inkey = try_get_value!("insert", "key", String, inkey);
        let scheme = &LIB_CFG.read_recursive().scheme[SETTINGS.read_recursive().current_scheme];
        let inkey = name(scheme, &inkey);
        let inval = args
            .get("value")
            .map(|v| v.to_owned())
            .unwrap_or(tera::Value::Null);
        map.insert(inkey.to_string(), inval);
    };

    Ok(Value::Object(map))
}

/// A Tera filter telling which natural language some provided textual data is
/// written in. It returns the ISO 639-1 code representations of the detected
/// language. This filter only acts on `String` types. All other types are
/// passed through. Returns the empty string in case the language can not be
/// detected reliably.
/// All input types must be `Value::String()`, output type is `Value::String(0)`
#[cfg(feature = "lang-detection")]
fn get_lang_filter<S: BuildHasher>(
    value: &Value,
    _args: &HashMap<String, Value, S>,
) -> TeraResult<Value> {
    let input = try_get_value!("get_lang", "value", String, value);
    let input = input.trim();
    // Return early if there is no input text.
    if input.is_empty() {
        return Ok(Value::String("".to_string()));
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
        _ => return Ok(Value::String("".to_string())),
    }
    .build();

    let detected_language = detector
        .detect_language_of(input)
        .map(|l| format!("{}", l.iso_code_639_1()))
        // If not languages can be detected, this returns the empty
        // string.
        .unwrap_or_default();
    log::debug!("Language '{}' in input detected.", detected_language);

    Ok(Value::String(detected_language))
}

#[cfg(not(feature = "lang-detection"))]
fn get_lang_filter<S: BuildHasher>(
    _value: &Value,
    _args: &HashMap<String, Value, S>,
) -> TeraResult<Value> {
    Ok(Value::String("".to_owned()))
}

/// A mapper for ISO 639 codes adding some region information, e.g.
/// `en` to `en-US` or `de` to `de-DE`. Configure the mapping with
/// `tmpl.filter.map_lang`.
/// An input value without mapping definition is passed through.
/// When the optional parameter `default` is given, e.g.
/// `map_lang(default=val)`, an empty input string is mapped to `val`.
/// All input types must be `Value::String()`, the output type is
/// `Value::String(0)`
fn map_lang_filter<S: BuildHasher>(
    value: &Value,
    args: &HashMap<String, Value, S>,
) -> TeraResult<Value> {
    let input = try_get_value!("map_lang", "value", String, value);

    let input = input.trim();
    if input.is_empty() {
        if let Some(default) = args.get("default") {
            let default = try_get_value!("map_lang", "default", String, default);
            return Ok(Value::String(default));
        } else {
            return Ok(Value::String("".to_owned()));
        };
    };
    let settings = SETTINGS.read_recursive();

    let res = if let Some(btm) = &settings.filter_map_lang_btmap {
        btm.get(input).map(|v| &v[..]).unwrap_or(input)
    } else {
        input
    };

    Ok(Value::String(res.to_owned()))
}

#[derive(Debug, Eq, PartialEq, Default)]
/// Represents the first heading in an HTML document.
struct FirstHtmlHeading<'a>(Cow<'a, str>);

impl<'a> FirstHtmlHeading<'a> {
    /// Parse the HTML input `i` and return the first HTML heading found.
    fn from(html: &'a str) -> Option<Self> {
        /// A pattern to search for HTML heading tags.
        const HTML_HEADING_OPENING_TAG: &[&str; 6] = &["<h1", "<h2", "<h3", "<h4", "<h5", "<h6"];

        /// A pattern to search for HTML heading tags.
        const HTML_HEADING_CLOSING_TAG: &[&str; 6] =
            &["</h1>", "</h2>", "</h3>", "</h4>", "</h5>", "</h6>"];

        let mut i = 0;
        let mut heading_start = None;
        let mut heading_end = None;

        // Find opening tag.
        while let Some(mut tag_start) = html[i..].find('<') {
            if let Some(mut tag_end) = html[i + tag_start..].find('>') {
                tag_end += 1;
                // Move on if there is another opening bracket.
                if let Some(new_start) = html[i + tag_start + 1..i + tag_start + tag_end].rfind('<')
                {
                    tag_start += new_start + 1;
                    tag_end -= new_start + 1;
                }

                // Is this a tag listed in `HTML_HEADING_OPENING_TAGS`?
                heading_start = HTML_HEADING_OPENING_TAG
                    .iter()
                    .any(|&pat| html[i + tag_start..i + tag_start + tag_end].starts_with(pat))
                    // Store the index after the opening tag.
                    .then_some(i + tag_start + tag_end);

                if heading_start.is_some() {
                    break;
                } else {
                    i += tag_start + tag_end;
                }
            } else {
                break;
            }
        }

        // Search for the closing tag.

        // Find closing tag.
        if let Some(mut i) = heading_start {
            while let Some(mut tag_start) = html[i..].find('<') {
                if let Some(mut tag_end) = html[i + tag_start..].find('>') {
                    tag_end += 1;
                    // Move on if there is another opening bracket.
                    if let Some(new_start) =
                        html[i + tag_start + 1..i + tag_start + tag_end].rfind('<')
                    {
                        tag_start += new_start + 1;
                        tag_end -= new_start + 1;
                    }

                    // Is this a tag listed in `HTML_HEADING_OPENING_TAGS`?
                    heading_end = HTML_HEADING_CLOSING_TAG
                        .iter()
                        .any(|&pat| html[i + tag_start..i + tag_start + tag_end].starts_with(pat))
                        // Store the index before the closing tag.
                        .then_some(i + tag_start);

                    if heading_end.is_some() {
                        break;
                    } else {
                        i += tag_start + tag_end;
                    }
                } else {
                    break;
                }
            }
        }

        // Get Heading slice.
        let mut heading = "";
        if let (Some(heading_start), Some(heading_end)) = (heading_start, heading_end) {
            heading = &html[heading_start..heading_end];
        }
        if heading.is_empty() {
            return None;
        }

        // Remove HTNL tags inside heading.
        let mut cleaned_heading = String::new();
        let mut inside_tag = false;
        for c in heading.chars() {
            if c == '<' {
                inside_tag = true;
            } else if c == '>' {
                inside_tag = false;
            } else if !inside_tag {
                cleaned_heading.push(c);
            }
        }
        if cleaned_heading.is_empty() {
            return None;
        }

        // Decode HTML entyties.
        let output: Cow<str> = if cleaned_heading == heading {
            html_escape::decode_html_entities(heading)
        } else {
            Cow::Owned(html_escape::decode_html_entities(&cleaned_heading).into_owned())
        };

        // Pack the output into newtype.
        if output.is_empty() {
            None
        } else {
            Some(FirstHtmlHeading(output))
        }
    }
}

#[derive(Debug, Eq, PartialEq, Default)]
/// Represents a hyperlink.
struct FirstHyperlink<'a> {
    text: Cow<'a, str>,
    dest: Cow<'a, str>,
    title: Cow<'a, str>,
}

impl<'a> FirstHyperlink<'a> {
    /// Parse the first markup formatted hyperlink and stores the result in `Self`.
    fn from(i: &'a str) -> Option<Self> {
        let mut hlinks = MarkupLink::new(i, false);
        hlinks
            .find_map(|l| match l.1 {
                Link::Text2Dest(te, de, ti) => Some((te, de, ti)),
                _ => None,
            })
            .map(|(text, dest, title)| FirstHyperlink { text, dest, title })
    }

    /// Parse the first markup formatted hyperlink and stores the result in `Self`.
    /// If this first link is an autolink, return `None`.
    fn from_picky(i: &'a str) -> Option<Self> {
        let mut hlinks = MarkupLink::new(i, false);

        hlinks.find_map(|l| {
            match l.1 {
                // Is this an autolink? Skip.
                // Email autolink? Skip
                Link::Text2Dest(text, dest, _) if text == dest => None,
                Link::Text2Dest(_, dest, _) if dest.to_lowercase().starts_with("mailto:") => None,
                Link::Text2Dest(text, _, _) if text.to_lowercase().starts_with("https:") => None,
                Link::Text2Dest(text, _, _) if text.to_lowercase().starts_with("http:") => None,
                Link::Text2Dest(text, _, _) if text.to_lowercase().starts_with("tpnote:") => None,
                Link::Text2Dest(text, dest, title) => Some(FirstHyperlink { text, dest, title }),
                _ => None,
            }
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
        // Array.
        let input = json!(["Ford", "BMW", "Fiat"]);
        let expected = "    - Ford\n    - BMW\n    - Fiat".to_string();
        let mut args = HashMap::new();
        args.insert("tab".to_string(), to_value(4).unwrap());
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
    fn test_name_filter() {
        //
        let result = name_filter(&to_value("fm_title").unwrap(), &HashMap::new());
        assert_eq!(result.unwrap(), to_value("title").unwrap());

        //
        let result = name_filter(&to_value("fm_unknown").unwrap(), &HashMap::new());
        assert_eq!(result.unwrap(), to_value("unknown").unwrap());
    }

    #[test]
    fn test_markup_to_html_filter() {
        //
        // Render verbatim text with markup hyperlinks to HTML.
        let input = json!("Hello World\n[link](<https://getreu.net>)");
        let expected = "<pre>Hello World\n\
            <a href=\"https://getreu.net\" title=\"\">\
            [link](&lt;https://getreu.net&gt;)</a></pre>"
            .to_string();

        let args = HashMap::new();
        assert_eq!(
            markup_to_html_filter(&input, &args).unwrap(),
            Value::String(expected)
        );

        // Render verbatim text with markup hyperlinks to HTML.
        let input = json!("Hello World\n[link](<https://getreu.net>)");
        let expected = "<pre>Hello World\n\
            <a href=\"https://getreu.net\" title=\"\">link</a></pre>"
            .to_string();
        let mut args = HashMap::new();
        // Select the "md" renderer.
        args.insert("extension".to_string(), to_value("txtnote").unwrap());

        assert_eq!(
            markup_to_html_filter(&input, &args).unwrap(),
            Value::String(expected)
        );

        //
        // Render Markdown to HTML.
        let input = json!("# Title\nHello World");
        let mut args = HashMap::new();
        // Select the "md" renderer.
        args.insert("extension".to_string(), to_value("md").unwrap());

        #[cfg(feature = "renderer")]
        let expected = "<h1>Title</h1>\n<p>Hello World</p>\n".to_string();
        #[cfg(not(feature = "renderer"))]
        let expected = "".to_string();

        assert_eq!(
            markup_to_html_filter(&input, &args).unwrap(),
            Value::String(expected)
        );
    }

    #[test]
    fn test_incr_sort_tag_filter() {
        let result = incr_sort_tag_filter(&to_value("dir/19-Note.md").unwrap(), &HashMap::new());
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), to_value("20").unwrap());

        let result = incr_sort_tag_filter(&to_value("Note.md").unwrap(), &HashMap::new());
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), to_value("").unwrap());

        let result = incr_sort_tag_filter(&to_value("29-Note.md").unwrap(), &HashMap::new());
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), to_value("30").unwrap());

        let result = incr_sort_tag_filter(&to_value("02-Note.md").unwrap(), &HashMap::new());
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), to_value("03").unwrap());

        let result = incr_sort_tag_filter(&to_value("cz-Note.md").unwrap(), &HashMap::new());
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), to_value("da").unwrap());

        let result = incr_sort_tag_filter(&to_value("2cz-Note.md").unwrap(), &HashMap::new());
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), to_value("2da").unwrap());

        // Too many letters, default string is ``.
        let result = incr_sort_tag_filter(&to_value("2acz-Note.md").unwrap(), &HashMap::new());
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), to_value("").unwrap());

        // No input.
        let mut args = HashMap::new();
        args.insert("default".to_string(), to_value("my default.md").unwrap());
        let result = incr_sort_tag_filter(&to_value("-Note.md").unwrap(), &args);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), to_value("my default.md").unwrap());

        // Too big.
        let mut args = HashMap::new();
        args.insert("default".to_string(), to_value("my default.md").unwrap());
        let result = incr_sort_tag_filter(&to_value("10000000000-Note.md").unwrap(), &args);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), to_value("my default.md").unwrap());

        // Too many digits.
        let mut args = HashMap::new();
        args.insert("default".to_string(), to_value("my default.md").unwrap());
        let result = incr_sort_tag_filter(&to_value("013-Note.md").unwrap(), &args);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), to_value("014").unwrap());

        // Too big.
        let mut args = HashMap::new();
        args.insert("default".to_string(), to_value("my default.md").unwrap());
        let result = incr_sort_tag_filter(&to_value("aaafbaz-Note.md").unwrap(), &args);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), to_value("my default.md").unwrap());

        // Too many digits.
        let mut args = HashMap::new();
        args.insert("default".to_string(), to_value("my default.md").unwrap());
        let result = incr_sort_tag_filter(&to_value("aaf-Note.md").unwrap(), &args);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), to_value("my default.md").unwrap());

        let mut args = HashMap::new();
        args.insert("default".to_string(), to_value("my default.md").unwrap());
        let result = incr_sort_tag_filter(&to_value("23-01-23-Note.md").unwrap(), &args);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), to_value("23-01-24").unwrap());
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
    fn test_remove_filter() {
        //
        let input = json!({"title": "my title", "subtitle": "my subtitle"});
        let mut args = HashMap::new();
        args.insert("key".to_string(), to_value("title").unwrap());
        let expected = json!({"subtitle": "my subtitle"});
        let result = remove_filter(&input, &args);
        //eprintln!("{:?}", result);
        assert_eq!(result.unwrap(), expected);

        //
        let input = json!({"title": "my title", "subtitle": "my subtitle"});
        let mut args = HashMap::new();
        args.insert("key".to_string(), to_value("nono").unwrap());
        let expected = json!({"title": "my title", "subtitle": "my subtitle"});
        let result = remove_filter(&input, &args);
        //eprintln!("{:?}", result);
        assert_eq!(result.unwrap(), expected);
    }

    #[test]
    fn test_insert_filter() {
        //
        let input = json!({"subtitle": "my subtitle"});
        let mut args = HashMap::new();
        args.insert("key".to_string(), to_value("fm_new").unwrap());
        args.insert("value".to_string(), to_value("my new").unwrap());
        let expected = json!({"new": "my new", "subtitle": "my subtitle"});
        let result = insert_filter(&input, &args);
        //eprintln!("{:?}", result);
        assert_eq!(result.unwrap(), expected);

        //
        let input = json!({"title": "my title", "subtitle": "my subtitle"});
        let mut args = HashMap::new();
        args.insert("key".to_string(), to_value("fm_title").unwrap());
        args.insert("value".to_string(), to_value("my replaced title").unwrap());
        let expected = json!({"title": "my replaced title", "subtitle": "my subtitle"});
        let result = insert_filter(&input, &args);
        //eprintln!("{:?}", result);
        assert_eq!(result.unwrap(), expected);

        //
        let input = json!({"title": "my title"});
        let mut args = HashMap::new();
        args.insert("key".to_string(), to_value("fm_new").unwrap());
        let expected = json!({"new": null, "title": "my title"});
        let result = insert_filter(&input, &args);
        //eprintln!("{:?}", result);
        assert_eq!(result.unwrap(), expected);
    }

    #[test]
    fn test_replace_emtpy_filter() {
        // Do not replace.
        let mut args = HashMap::new();
        args.insert("with".to_string(), to_value("new string").unwrap());
        let result = replace_empty_filter(&to_value("non empty string").unwrap(), &args);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), to_value("non empty string").unwrap());

        // Replace.
        let mut args = HashMap::new();
        args.insert("with".to_string(), to_value("new string").unwrap());
        let result = replace_empty_filter(&to_value("").unwrap(), &args);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), to_value("new string").unwrap());
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

        // `with`
        let mut args = HashMap::new();
        args.insert("with".to_string(), to_value("-").unwrap());
        args.insert("newline".to_string(), to_value(true).unwrap());
        let result = prepend_filter(&to_value("1. My first chapter").unwrap(), &args);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), to_value("\n-1. My first chapter").unwrap());
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
        assert_eq!(result.unwrap(), to_value("1. My first chapter-\n").unwrap());
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
    fn test_link_text_filter() {
        let args = HashMap::new();
        // Test Markdown link in clipboard.
        let input = r#"Some autolink: <tpnote:locallink.md>,
more autolinks: <tpnote:20>, <getreu@web.de>,
boring link text: [http://domain.com](http://getreu.net)
[Jens Getreu's blog](https://blog.getreu.net "My blog")
Some more text."#;

        let output = link_text_filter(&to_value(input).unwrap(), &args).unwrap_or_default();
        assert_eq!(output, "tpnote:locallink.md");

        // Test picky version also.

        let output = link_text_picky_filter(&to_value(input).unwrap(), &args).unwrap_or_default();
        assert_eq!(output, "Jens Getreu's blog");

        //
        let input = "[into\\_bytes](https://doc.rust-lang.org)";

        let output = link_text_filter(&to_value(input).unwrap(), &args).unwrap_or_default();
        assert_eq!(output, "into_bytes");

        // Test picky version also.

        let output = link_text_picky_filter(&to_value(input).unwrap(), &args).unwrap_or_default();
        assert_eq!(output, "into_bytes");
    }

    #[test]
    fn test_cut_filter() {
        let args = HashMap::new();
        // Test Markdown link in clipboard.
        let input = "Jens Getreu's blog";
        let output = cut_filter(&to_value(input).unwrap(), &args).unwrap_or_default();
        assert_eq!("Jens Getr", output);
    }

    #[test]
    fn test_first_html_heading() {
        // Test case: No heading in the HTML
        let html = "<p>No heading here</p>";
        assert_eq!(FirstHtmlHeading::from(html), None);

        // Test case: H1 heading
        let html = "<h1>Heading 1</h1>";
        assert_eq!(
            FirstHtmlHeading::from(html),
            Some(FirstHtmlHeading(Cow::Borrowed("Heading 1")))
        );

        // Test case: Nested tags within a heading
        let html = "<h2>Heading <span>with</span> nested tags</h2>";
        assert_eq!(
            FirstHtmlHeading::from(html),
            Some(FirstHtmlHeading(Cow::Borrowed("Heading with nested tags")))
        );

        // Test case: HTML entities within a heading
        let html = "<h3>Heading with &lt;html entities&gt;</h3>";
        assert_eq!(
            FirstHtmlHeading::from(html),
            Some(FirstHtmlHeading(Cow::Borrowed(
                "Heading with <html entities>"
            )))
        );

        // Test case: Multiple headings in the HTML
        let html = "<h4>First Heading</h4><h5>Second Heading</h5>";
        assert_eq!(
            FirstHtmlHeading::from(html),
            Some(FirstHtmlHeading(Cow::Borrowed("First Heading")))
        );

        // Test case: Heading without a closing tag
        let html = "<h1>Heading without closing tag";
        assert_eq!(FirstHtmlHeading::from(html), None);

        // Test case: Empty heading
        let html = "<h6></h6>";
        assert_eq!(FirstHtmlHeading::from(html), None);

        // Test case: Heading with attributes
        let html = "<h1 class=\"title\">Heading with attributes</h1>";
        assert_eq!(
            FirstHtmlHeading::from(html),
            Some(FirstHtmlHeading(Cow::Borrowed("Heading with attributes")))
        );
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
    fn test_html_heading_filter() {
        let args = HashMap::new();

        //
        // Test find first heading.
        let input = "Some text.<h1>Heading 1</h1>Get quickly\
            started writing notes.";
        let output = html_heading_filter(&to_value(input).unwrap(), &args).unwrap_or_default();
        // This string is shortened.
        assert_eq!("Heading 1", output);

        //
        let input = "Some text.<h1 style=\"font-size:60px;\">\
            Heading 1</h1>Get quickly\
            started writing notes.";
        let output = html_heading_filter(&to_value(input).unwrap(), &args).unwrap_or_default();
        // This string is shortened.
        assert_eq!("Heading 1", output);

        //
        let input = "Some text.<h2>Heading &amp;1</h2>Get quickly\
            started writing notes.";
        let output = html_heading_filter(&to_value(input).unwrap(), &args).unwrap_or_default();
        // This string is shortened.
        assert_eq!("Heading &1", output);

        //
        let input = "Some text.<p>No Heading 1</p>Get quickly\
            started writing notes.";
        let output = html_heading_filter(&to_value(input).unwrap(), &args).unwrap_or_default();
        // This string is shortened.
        assert_eq!("", output);

        //
        let input = "Some text.<h1>No Heading 1</p>Get quickly\
            started writing notes.";
        let output = html_heading_filter(&to_value(input).unwrap(), &args).unwrap_or_default();
        // This string is shortened.
        assert_eq!("", output);

        //
        let input = "Some text.<p>No Heading 1</h1>Get quickly\
            started writing notes.";
        let output = html_heading_filter(&to_value(input).unwrap(), &args).unwrap_or_default();
        // This string is shortened.
        assert_eq!("", output);

        //
        let input = "Some text.<p>No <h1>Heading 1</h1>Get quickly\
            started writing notes.";
        let output = html_heading_filter(&to_value(input).unwrap(), &args).unwrap_or_default();
        // This string is shortened.
        assert_eq!("Heading 1", output);

        //
        let input = "Some text.<p>No <h1>Heading<br> 1</h1>Get quickly\
            started writing notes.";
        let output = html_heading_filter(&to_value(input).unwrap(), &args).unwrap_or_default();
        // This string is shortened.
        assert_eq!("Heading 1", output);

        //
        let input = "<p>No <h1>Heading 1</h1> <h1>Heading 2</h1> text";
        let output = html_heading_filter(&to_value(input).unwrap(), &args).unwrap_or_default();
        // This string is shortened.
        assert_eq!("Heading 1", output);
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
    #[cfg(feature = "lang-detection")]
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
        use super::FirstHyperlink;
        // Stand alone Markdown link.
        let input = r#"abc[Homepage](https://blog.getreu.net "My blog")abc"#;
        let expected_output = FirstHyperlink {
            text: "Homepage".into(),
            dest: "https://blog.getreu.net".into(),
            title: "My blog".into(),
        };
        let output = FirstHyperlink::from(input);
        assert_eq!(expected_output, output.unwrap());

        // Markdown link reference.
        let input = r#"abc[Homepage][home]abc
                      [home]: https://blog.getreu.net "My blog""#;
        let expected_output = FirstHyperlink {
            text: "Homepage".into(),
            dest: "https://blog.getreu.net".into(),
            title: "My blog".into(),
        };
        let output = FirstHyperlink::from(input);
        assert_eq!(expected_output, output.unwrap());

        //
        // RestructuredText link
        let input = "abc`Homepage <https://blog.getreu.net>`_\nabc";
        let expected_output = FirstHyperlink {
            text: "Homepage".into(),
            dest: "https://blog.getreu.net".into(),
            title: "".into(),
        };
        let output = FirstHyperlink::from(input);
        assert_eq!(expected_output, output.unwrap());

        //
        // RestructuredText link ref
        let input = "abc `Homepage<home_>`_ abc\n.. _home: https://blog.getreu.net\nabc";
        let expected_output = FirstHyperlink {
            text: "Homepage".into(),
            dest: "https://blog.getreu.net".into(),
            title: "".into(),
        };
        let output = FirstHyperlink::from(input);
        assert_eq!(expected_output, output.unwrap());
    }
}
