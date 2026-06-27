//! Extends the built-in Tera filters.
//! All custom filters check the type of their input variables at runtime and
//! throw an error if the type is other than specified.
#[cfg(feature = "renderer")]
use crate::error::NoteError;
use crate::config::FILENAME_DOTFILE_MARKER;
use crate::config::LIB_CFG;
use crate::config::Scheme;
use crate::config::TMPL_VAR_FM_;
use crate::filename::NotePath;
use crate::filename::NotePathBuf;
use crate::filename::NotePathStr;
#[cfg(feature = "lang-detection")]
use crate::lingua::get_lang;
use crate::markup_language::InputConverter;
use crate::markup_language::MarkupLanguage;
use crate::settings::SETTINGS;
use parse_hyperlinks::iterator::MarkupLink;
use parse_hyperlinks::parser::Link;
use sanitize_filename_reader_friendly::sanitize;
use std::borrow::Cow;
use std::path::Path;
use std::path::PathBuf;
use std::sync::LazyLock;
use std::time::SystemTime;
use tera::value::Key;
use tera::{Kwargs, Map, State, TeraResult, Tera, Value};

/// Filter parameter of the `trunc_filter()` limiting the maximum length of
/// template variables. The filter is usually used to in the note's front matter
/// as title. For example: the title should not be too long, because it will end
/// up as part of the filename when the note is saved to disk. Filenames of some
/// operating systems are limited to 255 bytes.
#[cfg(not(test))]
const TRUNC_LEN_MAX: usize = 200;
#[cfg(test)]
pub const TRUNC_LEN_MAX: usize = 10;

/// Tera object with custom functions registered.
/// Converts Unix epoch seconds to (year, month, day) using the Gregorian
/// calendar. Based on https://howardhinnant.github.io/date_algorithms.html
fn unix_timestamp_to_ymd(secs: u64) -> (i32, u32, u32) {
    let z = (secs / 86400) as i64 + 719468;
    let era = if z >= 0 { z } else { z - 146096 } / 146097;
    let doe = z - era * 146097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    (y as i32, m as u32, d as u32)
}

/// Tera function returning the current time as Unix epoch seconds (u64).
/// Replaces tera v1's built-in `now()`.
fn now_function(_kwargs: Kwargs, _state: &State) -> TeraResult<Value> {
    let secs = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    Ok(Value::from(secs))
}

/// Tera filter formatting a Unix epoch seconds value using a strftime-style
/// format string. Supports `%Y`, `%m`, `%d`. Replaces tera v1's built-in
/// `date` filter.
fn date_filter(value: &Value, kwargs: Kwargs, _state: &State) -> TeraResult<Value> {
    let secs = value
        .as_u64()
        .ok_or_else(|| tera::Error::message("Filter 'date': value must be a unix timestamp"))?;
    let fmt_str = kwargs
        .get::<String>("format")?
        .unwrap_or_else(|| "%Y-%m-%d".to_string());
    let (year, month, day) = unix_timestamp_to_ymd(secs);
    let result = fmt_str
        .replace("%Y", &format!("{:04}", year))
        .replace("%m", &format!("{:02}", month))
        .replace("%d", &format!("{:02}", day));
    Ok(Value::from(result))
}

/// Converts any value to its string representation.
/// For strings, returns the raw string (no quotes).
fn as_str_filter(value: &Value, _kwargs: Kwargs, _state: &State) -> TeraResult<Value> {
    Ok(Value::from(value.to_string()))
}

pub static TERA: LazyLock<Tera> = LazyLock::new(|| {
    let mut tera = Tera::default();
    tera.register_filter("append", append_filter);
    tera.register_filter("file_copy_counter", file_copy_counter_filter);
    tera.register_filter("file_ext", file_ext_filter);
    tera.register_filter("file_name", file_name_filter);
    tera.register_filter("file_sort_tag", file_sort_tag_filter);
    tera.register_filter("file_stem", file_stem_filter);
    tera.register_filter("find_last_created_file", find_last_created_file);
    tera.register_filter("flatten_array", flatten_array_filter);
    tera.register_filter("get_lang", get_lang_filter);
    tera.register_filter("heading", heading_filter);
    tera.register_filter("html_heading", html_heading_filter);
    tera.register_filter("html_to_markup", html_to_markup_filter);
    tera.register_filter("incr_sort_tag", incr_sort_tag_filter);
    tera.register_filter("as_str", as_str_filter);
    tera.register_filter("insert", insert_filter);
    tera.register_filter("link_dest", link_dest_filter);
    tera.register_filter("link_text", link_text_filter);
    tera.register_filter("link_text_picky", link_text_picky_filter);
    tera.register_filter("link_title", link_title_filter);
    tera.register_filter("map_lang", map_lang_filter);
    tera.register_filter("markup_to_html", markup_to_html_filter);
    tera.register_filter("name", name_filter);
    tera.register_filter("prepend", prepend_filter);
    tera.register_filter("remove", remove_filter);
    tera.register_filter("replace_empty", replace_empty_filter);
    tera.register_filter("sanit", sanit_filter);
    tera.register_filter("to_html", to_html_filter);
    tera.register_filter("to_yaml", to_yaml_filter);
    tera.register_filter("trim_file_sort_tag", trim_file_sort_tag_filter);
    tera.register_filter("trunc", trunc_filter);
    tera.register_filter("date", date_filter);
    tera.register_function("now", now_function);
    tera
});

/// A filter converting any input `tera::Value` into a `tera::Value::String(s)`
/// with `s` being the YAML representation of the object. The input can be of
/// any type, the output type is always a `Value::String()`.
/// If the input type is `tera::Value::Object`, all top level keys starting with
/// `fm_` are localized (see `fm_var.localization`).
/// When the optional parameter `key='k'` is given, the input is regarded as
/// the corresponding value to this key.
/// The optional parameter `tab=n` indents the YAML values `n` characters to
/// the right of the first character of the key by inserting additional spaces
/// between the key and the value. When `tab=n` is given, it has precedence
/// over the default value, read from the configuration file variable
/// `tmpl.filter.to_yaml_tab`.
fn to_yaml_filter(
    val: &Value,
    kwargs: Kwargs,
    _state: &State,
) -> TeraResult<Value> {
    let scheme = &LIB_CFG.read_recursive().scheme[SETTINGS.read_recursive().current_scheme];

    let val_yaml = if let Some(k) = kwargs.get::<String>("key")? {
        let mut m = Map::new();
        let k = name(scheme, &k);
        m.insert(Key::from(k.to_owned()), val.clone());
        serde_yaml::to_string(&Value::from(m)).unwrap()
    } else if let Some(map) = val.as_map() {
        let mut m = Map::new();
        for (k, v) in map.iter() {
            let new_k = name(scheme, k.as_str().unwrap_or_default());
            m.insert(Key::from(new_k.to_owned()), v.clone());
        }
        serde_yaml::to_string(&Value::from(m)).unwrap()
    } else {
        serde_yaml::to_string(val).unwrap()
    };

    // Translate the empty set, into an empty string and return it.
    if val_yaml.trim_end() == "{}" {
        return Ok(Value::from(""));
    }

    // Formatting: adjust indent.
    let val_yaml: String = if let Some(tab) =
        kwargs.get::<u64>("tab")?.or_else(|| {
            let n = scheme.tmpl.filter.to_yaml_tab;
            if n == 0 { None } else { Some(n) }
        }) {
        val_yaml
            .lines()
            .map(|l| {
                let mut insert_pos = 0;
                let mut inserts_n = 0;
                if let Some(colpos) = l.find(": ") {
                    if let Some(key_pos) = l.find(char::is_alphabetic)
                        && key_pos < colpos
                            && l.find('\'').is_none_or(|p| p >= colpos)
                            && l.find("\"'").is_none_or(|p| p >= colpos)
                        {
                            insert_pos = colpos + ": ".len();
                            inserts_n = (tab as usize).saturating_sub(insert_pos);
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

    Ok(Value::from(val_yaml))
}

/// A filter that coverts a `tera::Value` tree into an HTML representation,
/// with following HTML tags:
/// * `Value::Object`: `<blockquote class="fm">` and `<div class="fm">`,
/// * `Value::Array`: `<ul class="fm">` and `<li class="fm">`,
/// * `Value::String`: no tag,
/// * Other non-string basic types: `<code class="fm">`.
///
/// The input can be of any type, the output type is `Value::String()`.
/// If the input type is `Value::Object`, all top level keys starting with
/// `fm_` are localized (see `fm_var.localization`).
/// Note: HTML templates escape HTML critical characters by default.
/// To use the `to_hmtl` filter in HTML templates, add a `safe` filter in last
/// position. This is no risk, as the `to_html` filter always escapes string
/// values automatically, regardless of the template type.
fn to_html_filter(
    value: &Value,
    _kwargs: Kwargs,
    _state: &State,
) -> TeraResult<Value> {
    fn tag_to_html(val: Value, is_root: bool, output: &mut String) {
        if let Some(a) = val.as_array() {
            output.push_str("<ul class=\"fm\">");
            for i in a.to_vec() {
                output.push_str("<li class=\"fm\">");
                tag_to_html(i, false, output);
                output.push_str("</li>");
            }
            output.push_str("</ul>");
        } else if let Some(s) = val.as_str() {
            output.push_str(&html_escape::encode_text(s));
        } else if let Some(map) = val.as_map() {
            output.push_str("<blockquote class=\"fm\">");
            let mut entries: Vec<_> = map.iter().collect();
            entries.sort_unstable_by(|(a, _), (b, _)| a.cmp(b));
            if is_root {
                let scheme =
                    &LIB_CFG.read_recursive().scheme[SETTINGS.read_recursive().current_scheme];
                for &(k, v) in &entries {
                    output.push_str("<div class=\"fm\">");
                    output.push_str(name(scheme, k.as_str().unwrap_or_default()));
                    output.push_str(": ");
                    tag_to_html(v.clone(), false, output);
                    output.push_str("</div>");
                }
            } else {
                for &(k, v) in &entries {
                    output.push_str("<div class=\"fm\">");
                    output.push_str(k.as_str().unwrap_or_default());
                    output.push_str(": ");
                    tag_to_html(v.clone(), false, output);
                    output.push_str("</div>");
                }
            }
            output.push_str("</blockquote>");
        } else {
            output.push_str("<code class=\"fm\">");
            output.push_str(&val.to_string());
            output.push_str("</code>");
        }
    }

    let mut html = String::new();
    tag_to_html(value.clone(), true, &mut html);

    Ok(Value::from(html))
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
fn name_filter(
    value: &Value,
    _kwargs: Kwargs,
    _state: &State,
) -> TeraResult<Value> {
    let input = value
        .as_str()
        .ok_or_else(|| tera::Error::message("Filter 'name': value must be a string"))?;

    // This replaces the `fm`-name in the key by the localized name.
    let scheme = &LIB_CFG.read_recursive().scheme[SETTINGS.read_recursive().current_scheme];
    let output = name(scheme, input);
    Ok(Value::from(output.to_string()))
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
fn html_to_markup_filter(
    value: &Value,
    kwargs: Kwargs,
    _state: &State,
) -> TeraResult<Value> {
    // Bring new methods into scope.
    use crate::html::HtmlStr;

    #[allow(unused_mut)]
    let mut buffer = value
        .as_str()
        .ok_or_else(|| tera::Error::message("Filter 'html_to_markup': value must be a string"))?
        .to_owned();

    let default = kwargs.get::<String>("default")?.unwrap_or_default();

    let firstline = buffer
        .lines()
        .next()
        .map(|l| l.trim_start().to_ascii_lowercase());
    if firstline.is_some_and(|l| l.as_str().has_html_start_tag()) {
        let extension = kwargs.get::<String>("extension")?.unwrap_or_default();

        let converter = InputConverter::build(&extension);
        buffer = match converter(buffer) {
            Ok(converted) if converted.is_empty() => default,
            Ok(converted) => converted,
            Err(e) => {
                log::info!("{}", e);
                default
            }
        };
    } else {
        buffer = default;
    }

    // Trim end without reallocation.
    buffer.truncate(buffer.trim_end().len());

    Ok(Value::from(buffer))
}

/// Takes the markup formatted input and renders it to HTML.
/// The parameter file `extension` indicates in what Markup
/// language the input is written.
/// When `extension` is not given or known, the renderer defaults to
/// `MarkupLanguage::Unknown`.
/// The input types must be `Value::String` and the output type is
/// `Value::String()`
fn markup_to_html_filter(
    value: &Value,
    kwargs: Kwargs,
    _state: &State,
) -> TeraResult<Value> {
    let input = value
        .as_str()
        .ok_or_else(|| tera::Error::message("Filter 'markup_to_html': value must be a string"))?;

    let markup_language = if let Some(ext) = kwargs.get::<String>("extension")? {
        let ml = MarkupLanguage::from(ext.as_str());
        if ml.is_some() { ml } else { MarkupLanguage::Unkown }
    } else {
        MarkupLanguage::Unkown
    };

    // Render the markup language. When the renderer feature is enabled,
    // catch panics (e.g. unsupported markup elements) and Err() returns,
    // mapping each to its own NoteError variant before propagating as a
    // Tera error.  Without the renderer feature neither panics nor Err()
    // returns can occur, so a plain call suffices.
    #[cfg(feature = "renderer")]
    let html_output = {
        let renderer = format!("{:?}", markup_language);
        match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            markup_language.render(input)
        })) {
            Ok(Ok(html)) => html,
            Ok(Err(e)) => {
                return Err(tera::Error::message(format!(
                    "markup_to_html: {}",
                    NoteError::RenderError { renderer, msg: e.to_string() }
                )))
            }
            Err(payload) => {
                let msg = payload
                    .downcast_ref::<&str>()
                    .map(|s| s.to_string())
                    .or_else(|| payload.downcast_ref::<String>().cloned())
                    .unwrap_or_else(|| "unknown".to_string());
                return Err(tera::Error::message(format!(
                    "markup_to_html: {}",
                    NoteError::RenderPanic { renderer, msg }
                )))
            }
        }
    };
    #[cfg(not(feature = "renderer"))]
    let html_output = markup_language
        .render(input)
        .map_err(|e| tera::Error::message(e.to_string()))?;

    Ok(Value::from(html_output))
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
fn sanit_filter(
    value: &Value,
    _kwargs: Kwargs,
    _state: &State,
) -> TeraResult<Value> {
    let input = value
        .as_str()
        .ok_or_else(|| tera::Error::message("Filter 'sanit': value must be a string"))?;

    // Check if this is a usual dotfile filename.
    let is_dotfile = input.starts_with(FILENAME_DOTFILE_MARKER)
        && PathBuf::from(&*input).has_wellformed_filename();

    // Sanitize string.
    let mut res = sanitize(&input);

    // If `FILNAME_DOTFILE_MARKER` was stripped, prepend one.
    if is_dotfile && !res.starts_with(FILENAME_DOTFILE_MARKER) {
        res.insert(0, FILENAME_DOTFILE_MARKER);
    }

    Ok(Value::from(res))
}

/// A Tera filter that searches for the first Markdown or ReStructuredText link
/// in the input stream and returns the link's name (link text).
/// If not found, it returns the empty string.
/// The input type must be `Value::String` and the output type is
/// `Value::String()`
fn link_text_filter(
    value: &Value,
    _kwargs: Kwargs,
    _state: &State,
) -> TeraResult<Value> {
    let input = value
        .as_str()
        .ok_or_else(|| tera::Error::message("Filter 'link_text': value must be a string"))?;

    let hyperlink = FirstHyperlink::from(input).unwrap_or_default();

    Ok(Value::from(hyperlink.text.to_string()))
}

/// A Tera filter that searches for the first Markdown or ReStructuredText link
/// in the input stream and returns the link's URL.
/// If not found, it returns the empty string.
/// The input type must be `Value::String` and the output type is
/// `Value::String()`
fn link_dest_filter(
    value: &Value,
    _kwargs: Kwargs,
    _state: &State,
) -> TeraResult<Value> {
    let p = value
        .as_str()
        .ok_or_else(|| tera::Error::message("Filter 'link_dest': value must be a string"))?;

    let hyperlink = FirstHyperlink::from(p).unwrap_or_default();

    Ok(Value::from(hyperlink.dest.to_string()))
}

/// A Tera filter that searches for the first Markdown or ReStructuredText link
/// in the input stream and returns the link's text's name (link text).
/// Unlike the filter `link_dest`, it does not necessarily return the first
/// finding. For example, it skips autolinks, local links and links
/// with some URL in the link text.
/// If not found, it returns the empty string.
/// The input type must be `Value::String` and the output type is
/// `Value::String()`
fn link_text_picky_filter(
    value: &Value,
    _kwargs: Kwargs,
    _state: &State,
) -> TeraResult<Value> {
    let p = value
        .as_str()
        .ok_or_else(|| tera::Error::message("Filter 'link_text_picky': value must be a string"))?;

    let hyperlink = FirstHyperlink::from_picky(p).unwrap_or_default();

    Ok(Value::from(hyperlink.text.to_string()))
}

/// A Tera filter that searches for the first Markdown or ReStructuredText link
/// in the input stream and returns the link's title.
/// If not found, it returns the empty string.
/// The input type must be `Value::String` and the output type is
/// `Value::String()`
fn link_title_filter(
    value: &Value,
    _kwargs: Kwargs,
    _state: &State,
) -> TeraResult<Value> {
    let p = value
        .as_str()
        .ok_or_else(|| tera::Error::message("Filter 'link_title': value must be a string"))?;

    let hyperlink = FirstHyperlink::from(p).unwrap_or_default();

    Ok(Value::from(hyperlink.title.to_string()))
}

/// A Tera filter that searches for the first HTML heading
/// in the HTML input stream and returns the heading text.
/// If not found, it returns the empty string.
/// The input type must be `Value::String` and the output type is
/// `Value::String()`
fn html_heading_filter(
    value: &Value,
    _kwargs: Kwargs,
    _state: &State,
) -> TeraResult<Value> {
    let p = value
        .as_str()
        .ok_or_else(|| tera::Error::message("Filter 'html_heading': value must be a string"))?;

    let html_heading = FirstHtmlHeading::from(p).unwrap_or_default();

    Ok(Value::from(html_heading.0.to_string()))
}

/// A Tera filter that truncates the input stream and returns the
/// max `TRUNC_LEN_MAX` bytes of valid UTF-8.
/// The input type must be `Value::String` and the output type is
/// `Value::String()`
fn trunc_filter(
    value: &Value,
    _kwargs: Kwargs,
    _state: &State,
) -> TeraResult<Value> {
    let input = value
        .as_str()
        .ok_or_else(|| tera::Error::message("Filter 'trunc': value must be a string"))?;

    let mut short = "";
    for i in (0..TRUNC_LEN_MAX).rev() {
        if let Some(s) = input.get(..i) {
            short = s;
            break;
        }
    }
    Ok(Value::from(short.to_owned()))
}

/// A Tera filter that returns the first line or the first sentence of the input
/// stream.
/// The input type must be `Value::String` and the output type is
/// `Value::String()`
fn heading_filter(
    value: &Value,
    _kwargs: Kwargs,
    _state: &State,
) -> TeraResult<Value> {
    let p = value
        .as_str()
        .ok_or_else(|| tera::Error::message("Filter 'heading': value must be a string"))?;
    let p = p.trim_start();

    // Find the first heading, can finish with `. `, `.\n` or `.\r\n` on Windows.
    let mut index = p.len();

    if let Some(i) = p.find(". ")
        && i < index {
            index = i;
        }
    if let Some(i) = p.find(".\n")
        && i < index {
            index = i;
        }
    if let Some(i) = p.find(".\r\n")
        && i < index {
            index = i;
        }
    if let Some(i) = p.find('!')
        && i < index {
            index = i;
        }
    if let Some(i) = p.find('?')
        && i < index {
            index = i;
        }
    if let Some(i) = p.find("\n\n")
        && i < index {
            index = i;
        }
    if let Some(i) = p.find("\r\n\r\n")
        && i < index {
            index = i;
        }
    let content_heading = p[0..index].to_string();

    Ok(Value::from(content_heading))
}

/// A Tera filter that takes a path and extracts the tag of the filename.
/// The input type must be `Value::String` and the output type is
/// `Value::String()`
fn file_sort_tag_filter(
    value: &Value,
    _kwargs: Kwargs,
    _state: &State,
) -> TeraResult<Value> {
    let p = value
        .as_str()
        .ok_or_else(|| tera::Error::message("Filter 'file_sort_tag': value must be a string"))?;
    let p = PathBuf::from(p);
    let (tag, _, _, _, _) = p.disassemble();

    Ok(Value::from(tag.to_owned()))
}

/// A Tera filter that takes a path and extracts its last element.
/// This function trims the `sort_tag` if present.
/// The input type must be `Value::String` and the output type is
/// `Value::String()`
fn trim_file_sort_tag_filter(
    value: &Value,
    _kwargs: Kwargs,
    _state: &State,
) -> TeraResult<Value> {
    let input = value
        .as_str()
        .ok_or_else(|| tera::Error::message("Filter 'trim_file_sort_tag': value must be a string"))?;
    let input = PathBuf::from(input);
    let (_, fname, _, _, _) = input.disassemble();

    Ok(Value::from(fname.to_owned()))
}

/// A Tera filter that takes a path and extracts its file stem,
/// in other words: the filename without `sort_tag`, `file_copy_counter`
/// and `extension`.
/// The input type must be `Value::String` and the output type is
/// `Value::String()`
fn file_stem_filter(
    value: &Value,
    _kwargs: Kwargs,
    _state: &State,
) -> TeraResult<Value> {
    let input = value
        .as_str()
        .ok_or_else(|| tera::Error::message("Filter 'file_stem': value must be a string"))?;
    let input = PathBuf::from(input);
    let (_, _, stem, _, _) = input.disassemble();

    Ok(Value::from(stem.to_owned()))
}

/// A Tera filter that takes a path and extracts its copy counter,
/// or, to put it another way: the filename without `sort_tag`, `file_stem`
/// and `file_ext` (and their separators). If the filename contains a
/// `copy_counter=n`, the returned JSON value variant is `Value::Number(n)`.
/// If there is no copy counter in the input, the output is `Value::Number(0)`.
/// The input type must be `Value::String` and the output type is
/// `Value::Number()`
fn file_copy_counter_filter(
    value: &Value,
    _kwargs: Kwargs,
    _state: &State,
) -> TeraResult<Value> {
    let input = value
        .as_str()
        .ok_or_else(|| tera::Error::message("Filter 'file_copy_counter': value must be a string"))?;
    let input = PathBuf::from(input);
    let (_, _, _, copy_counter, _) = input.disassemble();
    let copy_counter = copy_counter.unwrap_or(0);

    Ok(Value::from(copy_counter))
}

/// A Tera filter that takes a path and extracts its filename without
/// file extension. The filename may contain a sort-tag, a copy-counter and
/// separators. The input type must be `Value::String` and the output type is
/// `Value::String()`
fn file_name_filter(
    value: &Value,
    _kwargs: Kwargs,
    _state: &State,
) -> TeraResult<Value> {
    let p = value
        .as_str()
        .ok_or_else(|| tera::Error::message("Filter 'file_name': value must be a string"))?;

    let filename = Path::new(p)
        .file_name()
        .unwrap_or_default()
        .to_str()
        .unwrap_or_default()
        .to_owned();

    Ok(Value::from(filename))
}

/// A Tera filter that replace the input string with the parameter `with`, but
/// only if the input stream is empty, e.g.:
///
/// * `Value::Null` or
/// * `Value::String("")`, or
/// * `Value::Array([])`, or
/// * the array contains only empty strings.
///
/// The parameter `with` can be any `Value` type.
fn replace_empty_filter(
    value: &Value,
    kwargs: Kwargs,
    _state: &State,
) -> TeraResult<Value> {
    let is_empty = if value.is_none() {
        true
    } else if let Some(s) = value.as_str() {
        s.is_empty()
    } else if let Some(values) = value.as_array() {
        values.is_empty()
            || values
                .iter()
                .map(|v| v.as_str())
                .all(|s| s.is_some_and(|s| s.is_empty()))
    } else {
        false
    };

    if is_empty {
        Ok(kwargs.get::<Value>("with")?.unwrap_or_else(|| value.clone()))
    } else {
        Ok(value.clone())
    }
}

/// A Tera filter that prepends the string parameter `with`, but only if the
/// input stream is not empty.
/// In addition, the flag `newline` inserts a newline character at end of the
/// result. In case the input stream is empty nothing is appended.
/// When called with the strings parameter `with_sort_tag`, the filter
/// prepends the sort-tag and all necessary sort-tag separator characters,
/// regardless whether the input stream in empty or not.
/// The input type, and the type of the parameter `with` and `with_sort_tag`
/// must be `Value::String`. The parameter `newline` must be a `Value::Bool` and
/// the output type is `Value::String()`.
fn prepend_filter(
    value: &Value,
    kwargs: Kwargs,
    _state: &State,
) -> TeraResult<Value> {
    let input = value
        .as_str()
        .ok_or_else(|| tera::Error::message("Filter 'prepend': value must be a string"))?;

    let mut res = input.to_owned();

    if let Some(with) = kwargs.get::<String>("with")? {
        let mut s = String::new();
        if !res.is_empty() {
            s.push_str(&with);
            s.push_str(&res);
            res = s;
        };
    } else if let Some(sort_tag) = kwargs.get::<String>("with_sort_tag")? {
        res = PathBuf::from_disassembled(&sort_tag, &res, None, "")
            .to_str()
            .unwrap_or_default()
            .to_string();
    };

    if let Some(newline) = kwargs.get::<bool>("newline")?
        && newline && !res.is_empty() {
            let mut s = String::new();
            s.push('\n');
            s.push_str(&res);
            res = s;
        };

    Ok(Value::from(res))
}

/// A Tera filter that appends the string parameter `with`. In addition, the
/// flag `newline` inserts a newline character at end of the result. In
/// case the input stream is empty, nothing is appended.
/// The input type, and the type of the parameter `with` must be
/// `Value::String`. The parameter `newline` must be a `Value::Bool` and the
/// output type is `Value::String()`.
fn append_filter(
    value: &Value,
    kwargs: Kwargs,
    _state: &State,
) -> TeraResult<Value> {
    let input = value
        .as_str()
        .ok_or_else(|| tera::Error::message("Filter 'append': value must be a string"))?;

    if input.is_empty() {
        return Ok(Value::from(""));
    }

    let mut res = input.to_owned();
    if let Some(with) = kwargs.get::<String>("with")? {
        res.push_str(&with);
    };

    if let Some(newline) = kwargs.get::<bool>("newline")? {
        if newline && !res.is_empty() {
            res.push('\n');
        }
    };

    Ok(Value::from(res))
}

/// A Tera filter that takes a path and extracts its file extension.
/// The input type must be `Value::String()`, the output type is
/// `Value::String()`.
fn file_ext_filter(
    value: &Value,
    _kwargs: Kwargs,
    _state: &State,
) -> TeraResult<Value> {
    let p = value
        .as_str()
        .ok_or_else(|| tera::Error::message("Filter 'file_ext': value must be a string"))?;

    let ext = Path::new(p)
        .extension()
        .unwrap_or_default()
        .to_str()
        .unwrap_or_default()
        .to_owned();

    Ok(Value::from(ext))
}

/// A Tera filter that takes a directory path and returns the alphabetically
/// last sort-tag of all Tp-Note documents in that directory.
/// The filter returns the empty string if none was found.
/// The input type must be `Value::String()`, the output type is
/// `Value::String()`.
fn find_last_created_file(
    value: &Value,
    _kwargs: Kwargs,
    _state: &State,
) -> TeraResult<Value> {
    let p_str = value
        .as_str()
        .ok_or_else(|| tera::Error::message("Filter 'find_last_created_file': value must be a string"))?;

    let p = Path::new(p_str);
    let last = match p.find_last_created_file() {
        Some(filename) => Path::join(p, Path::new(&filename))
            .to_str()
            .unwrap()
            .to_string(),
        None => String::new(),
    };

    Ok(Value::from(last.to_string()))
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
/// on disk already. If this is the case, a subcounter is appended to the
/// resulting sort-tag.
/// All input types are `Value::String`. The output type is `Value::String()`.
fn incr_sort_tag_filter(
    value: &Value,
    kwargs: Kwargs,
    _state: &State,
) -> TeraResult<Value> {
    let input = value
        .as_str()
        .ok_or_else(|| tera::Error::message("Filter 'incr_sort_tag': value must be a string"))?;

    let default = kwargs.get::<String>("default")?.unwrap_or_default();

    let (input_dir, filename) = input.rsplit_once(['/', '\\']).unwrap_or(("", input));
    let (input_sort_tag, _, is_sequential) = filename.split_sort_tag(false);

    if input_sort_tag.is_empty() || !is_sequential {
        return Ok(Value::from(default));
    }

    // Start analyzing the input.
    let (prefix, digits) = match input_sort_tag.rfind(|c: char| !c.is_ascii_digit()) {
        Some(idx) => (&input_sort_tag[..idx + 1], &input_sort_tag[idx + 1..]),
        None => ("", input_sort_tag),
    };

    // Search for digits
    let mut output_sort_tag = if !digits.is_empty() {
        // Return early if this number is too big.
        const DIGITS_MAX: usize = u32::MAX.ilog10() as usize; // 9
        if digits.len() > DIGITS_MAX {
            return Ok(Value::from(default));
        }

        // Convert string to n base 10.
        let mut n = match digits.parse::<u32>() {
            Ok(n) => n,
            _ => return Ok(Value::from(default)),
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
                return Ok(Value::from(default));
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

    Ok(Value::from(output_sort_tag))
}

/// A Tera filter that takes a map of variables/values and removes a key/value
/// pair with the parameter `remove(key="<var-name>").
/// The input type must be `Value::Object()`, the parameter must be
/// `Value::String()` and the output type is `Value::Object()`.
fn remove_filter(
    value: &Value,
    kwargs: Kwargs,
    _state: &State,
) -> TeraResult<Value> {
    let mut map = value
        .clone()
        .into_map()
        .ok_or_else(|| tera::Error::message("Filter 'remove': value must be a map"))?;

    if let Some(outkey) = kwargs.get::<String>("key")? {
        let _ = map.remove(&Key::from(outkey));
    };

    Ok(Value::from(map))
}

/// A Tera filter that takes a map of key/values and inserts a key/value pair
/// with the parameters `insert(key="<var-name>", value=<var-value>). If the
/// variable exists in the map already, its value is replaced.
/// The input type must be `Value::Object()`, the `key` parameter must be a
/// `Value::String()` and the output type is `Value::Object()`.
fn insert_filter(
    value: &Value,
    kwargs: Kwargs,
    _state: &State,
) -> TeraResult<Value> {
    let mut map = value
        .clone()
        .into_map()
        .ok_or_else(|| tera::Error::message("Filter 'insert': value must be a map"))?;

    if let Some(inkey) = kwargs.get::<String>("key")? {
        let scheme = &LIB_CFG.read_recursive().scheme[SETTINGS.read_recursive().current_scheme];
        let inkey = name(scheme, &inkey).to_owned();
        let inval = kwargs.get::<Value>("value")?.unwrap_or(Value::none());
        map.insert(Key::from(inkey), inval);
    };

    Ok(Value::from(map))
}

/// A Tera filter telling in which natural language(s) the input text is
/// written. It returns an array of ISO 639-1 code representations listing the
/// detected languages. The input type must be a `Value::String`. The output
/// type is `Value::Array(<Vec<Value::String>>)`. If no language can be
/// reliably identified, the output is the empty array `Value::Array(vec![])`.
#[cfg(feature = "lang-detection")]
fn get_lang_filter(
    value: &Value,
    _kwargs: Kwargs,
    _state: &State,
) -> TeraResult<Value> {
    let input = value
        .as_str()
        .ok_or_else(|| tera::Error::message("Filter 'get_lang': value must be a string"))?;

    let l = get_lang(input).map_err(|e| tera::Error::message(e.to_string()))?;
    Ok(Value::from(l))
}

#[cfg(not(feature = "lang-detection"))]
fn get_lang_filter(
    _value: &Value,
    _kwargs: Kwargs,
    _state: &State,
) -> TeraResult<Value> {
    Ok(Value::from(""))
}

/// A mapper that is usually used to convert ISO 639 codes to IETF language tags
/// by appending some region information, e.g.
/// `en` to `en-US` or `de` to `de-DE`. Configure the mapping with
/// `tmpl.filter.map_lang`:
///
/// `Fn: Array(<Vec<String>>) -> Value::Array(<Vec<String>>)`
///
/// The input and output type is `Value::Array(<Vec<String>>)`.
/// If the input `<String>` is a key in `tmpl.filter.map_lang`, it is replaced
/// with the corresponding value. If the input does not correspond to a key in
/// `tmpl.filter.map_lang`, it is passed through as such.
/// In case the optional parameter `default` (type `Value::String`) is given,
/// e.g. `map_lang(default="abc")`, then an empty input array is mapped to
/// `Value::Array(Vec::from("abc"))`.
fn map_lang_filter(
    value: &Value,
    kwargs: Kwargs,
    _state: &State,
) -> TeraResult<Value> {
    // Type check.
    if !value
        .as_array()
        .is_some_and(|a| a.iter().all(|s| s.is_string()))
    {
        return Err(tera::Error::message("input must be an array of strings"));
    }

    // In `input` is empty return default.
    if value
        .as_array()
        .is_some_and(|a| a.is_empty() || a.iter().all(|v| v.as_str().is_some_and(|s| s.is_empty())))
    {
        return Ok(kwargs
            .get::<Value>("default")?
            .map(|v| Value::from(vec![v]))
            .unwrap_or_else(|| value.clone()));
    }

    // Set up converter.
    let settings = SETTINGS.read_recursive();
    let convert = |v: Value| {
        if let (Some(s), Some(btm)) = (v.as_str(), &settings.map_lang_filter_btmap) {
            btm.get(s)
                .map(|new_v| Value::from(new_v.as_str()))
                .unwrap_or(v)
        } else {
            v
        }
    };
    // Do conversion.
    let res = if let Some(a) = value.as_array() {
        Value::from(a.iter().cloned().map(convert).collect::<Vec<_>>())
    } else {
        value.clone()
    };

    Ok(res)
}

/// The input must be of type `Value::Array(<Vec<a>>)`. If the array has
/// exactly one element, then the array is flattened to `<a>` otherwise the
/// input is passed through.
fn flatten_array_filter(
    value: &Value,
    _kwargs: Kwargs,
    _state: &State,
) -> TeraResult<Value> {
    // Type check.
    if !value.is_array() {
        return Err(tera::Error::message("input must be of type array"));
    }

    // If the array has exactly one element, flatten it.
    match value.as_array() {
        Some(v) if v.len() == 1 => Ok(v[0].clone()),
        _ => Ok(value.clone()),
    }
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

        // Remove HTML tags inside heading.
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

        // Decode HTML entities.
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
    use std::collections::BTreeMap;
    use tera::{Kwargs, State};

    #[test]
    fn test_to_yaml_filter() {
        let ctx = tera::Context::new();
        let st = State::new(&ctx);

        // No key, the input is of type map.
        let input = Value::from_serializable(&json!({"number_type": 123}));
        let expected = "number_type:  123".to_string();
        assert_eq!(
            to_yaml_filter(&input, Kwargs::default(), &st).unwrap(),
            Value::from(expected)
        );

        //
        // The key is `author`, the value is a string.
        let input = Value::from("Getreu");
        let expected = "author:       Getreu".to_string();
        assert_eq!(
            to_yaml_filter(&input, Kwargs::from([("key", Value::from("author"))]), &st).unwrap(),
            Value::from(expected)
        );

        //
        // The key is `my`, the value is a map.
        let input =
            Value::from_serializable(&json!({"author": ["Getreu: Noname", "Jens: Noname"]}));
        let expected = "my:\n  author:\n  - 'Getreu: Noname'\n  - 'Jens: Noname'".to_string();
        assert_eq!(
            to_yaml_filter(&input, Kwargs::from([("key", Value::from("my"))]), &st).unwrap(),
            Value::from(expected)
        );

        //
        // The key is `my`, the value is a map.
        let input = Value::from_serializable(&json!({"number_type": 123}));
        let expected = "my:\n  number_type: 123".to_string();
        assert_eq!(
            to_yaml_filter(&input, Kwargs::from([("key", Value::from("my"))]), &st).unwrap(),
            Value::from(expected)
        );

        //
        // The key is `my`, `tab` is 10, the value is a map.
        let input = Value::from_serializable(&json!({"num": 123}));
        let expected = "my:\n  num:    123".to_string();
        assert_eq!(
            to_yaml_filter(
                &input,
                Kwargs::from([("key", Value::from("my")), ("tab", Value::from(10u64))]),
                &st,
            )
            .unwrap(),
            Value::from(expected)
        );

        //
        // Empty input.
        let input = Value::from_serializable(&json!({}));
        let expected = "".to_string();
        assert_eq!(
            to_yaml_filter(&input, Kwargs::from([("tab", Value::from(10u64))]), &st).unwrap(),
            Value::from(expected)
        );

        //
        // Empty input with key.
        let input = Value::from_serializable(&json!({}));
        let expected = "my:       {}".to_string();
        assert_eq!(
            to_yaml_filter(
                &input,
                Kwargs::from([("key", Value::from("my")), ("tab", Value::from(10u64))]),
                &st,
            )
            .unwrap(),
            Value::from(expected)
        );

        //
        // Simple input string, no map.
        let input = Value::from("my str");
        let expected = "my str".to_string();
        assert_eq!(
            to_yaml_filter(&input, Kwargs::from([("tab", Value::from(10u64))]), &st).unwrap(),
            Value::from(expected)
        );

        //
        // Simple input string, no map.
        let input = Value::from("my: str");
        let expected = "'my: str'".to_string();
        assert_eq!(
            to_yaml_filter(&input, Kwargs::from([("tab", Value::from(10u64))]), &st).unwrap(),
            Value::from(expected)
        );

        //
        // Array.
        let input = Value::from_serializable(&json!(["Ford", "BMW", "Fiat"]));
        let expected = "    - Ford\n    - BMW\n    - Fiat".to_string();
        assert_eq!(
            to_yaml_filter(&input, Kwargs::from([("tab", Value::from(4u64))]), &st).unwrap(),
            Value::from(expected)
        );

        //
        // Simple input number, no map.
        let input = Value::from_serializable(&json!(9876));
        let expected = "9876".to_string();
        assert_eq!(
            to_yaml_filter(&input, Kwargs::from([("tab", Value::from(10u64))]), &st).unwrap(),
            Value::from(expected)
        );
    }

    #[test]
    fn test_to_html_filter() {
        let ctx = tera::Context::new();
        let st = State::new(&ctx);

        //
        let input = Value::from_serializable(&json!(["Hello", "World", 123]));
        let expected = "<ul class=\"fm\"><li class=\"fm\">Hello</li>\
            <li class=\"fm\">World</li><li class=\"fm\">\
            <code class=\"fm\">123</code></li></ul>"
            .to_string();
        assert_eq!(
            to_html_filter(&input, Kwargs::default(), &st).unwrap(),
            Value::from(expected)
        );

        //
        let input = Value::from_serializable(&json!({
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
        }));
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
        assert_eq!(
            to_html_filter(&input, Kwargs::default(), &st).unwrap(),
            Value::from(expected)
        );
    }

    #[test]
    fn test_name_filter() {
        let ctx = tera::Context::new();
        let st = State::new(&ctx);

        //
        let result = name_filter(&Value::from("fm_title"), Kwargs::default(), &st);
        assert_eq!(result.unwrap(), Value::from("title"));

        //
        let result = name_filter(&Value::from("fm_unknown"), Kwargs::default(), &st);
        assert_eq!(result.unwrap(), Value::from("unknown"));
    }

    #[test]
    fn test_markup_to_html_filter() {
        let ctx = tera::Context::new();
        let st = State::new(&ctx);

        //
        // Render verbatim text with the `parse-hyperlinks` crate to HTML.
        let input = Value::from("Hello World\n[link](<https://getreu.net>)");
        let expected = "<pre>Hello World\n\
            <a href=\"https://getreu.net\" title=\"\">\
            [link](&lt;https://getreu.net&gt;)</a></pre>"
            .to_string();
        assert_eq!(
            markup_to_html_filter(&input, Kwargs::default(), &st).unwrap(),
            Value::from(expected)
        );

        // Render verbatim text with the `parse-hyperlinks` crate to HTML.
        let input = Value::from("Hello World\n[link](<https://getreu.net>)");
        let expected = "<pre>Hello World\n\
            <a href=\"https://getreu.net\" title=\"\">link</a></pre>"
            .to_string();
        // Select the "txtnote" renderer.
        assert_eq!(
            markup_to_html_filter(
                &input,
                Kwargs::from([("extension", Value::from("txtnote"))]),
                &st,
            )
            .unwrap(),
            Value::from(expected)
        );

        //
        // Render Markdown to HTML.
        let input = Value::from("# Title\nHello World");

        #[cfg(feature = "renderer")]
        let expected = "<h1>Title</h1>\n<p>Hello World</p>\n".to_string();
        #[cfg(not(feature = "renderer"))]
        let expected = "".to_string();

        assert_eq!(
            markup_to_html_filter(
                &input,
                Kwargs::from([("extension", Value::from("md"))]),
                &st,
            )
            .unwrap(),
            Value::from(expected)
        );

        //
        // Render valid ReStructuredText to HTML (happy path).
        #[cfg(feature = "renderer")]
        {
            let input = Value::from("`Link text <https://domain.invalid/>`_");
            let expected = "<p><a href=\"https://domain.invalid/\">Link text</a></p>";
            assert_eq!(
                markup_to_html_filter(
                    &input,
                    Kwargs::from([("extension", Value::from("rst"))]),
                    &st,
                )
                .unwrap(),
                Value::from(expected.to_string())
            );
        }
    }

    /// RST renderer panics on unsupported elements (e.g. unresolved substitution
    /// references); verify that `markup_to_html_filter` catches the panic and
    /// returns a `RenderPanic` error instead of unwinding the caller.
    #[test]
    #[cfg(feature = "renderer")]
    fn test_markup_to_html_filter_render_panic() {
        let ctx = tera::Context::new();
        let st = State::new(&ctx);

        // An unresolved substitution reference (|undefined|) leaves a
        // `SubstitutionReference` node in the AST that rst_renderer does not
        // implement, causing `unimplemented!()` to fire.
        let input = Value::from("The |undefined| substitution reference.");
        let result = markup_to_html_filter(
            &input,
            Kwargs::from([("extension", Value::from("rst"))]),
            &st,
        );
        assert!(result.is_err(), "expected Err from panicking RST renderer");

        // Build the expected prefix: "markup_to_html: " sentinel followed by the
        // NoteError::RenderPanic format string. msg is left empty to produce the
        // invariant prefix; the panic payload ("not implemented") follows it.
        let expected_prefix = format!(
            "markup_to_html: {}",
            NoteError::RenderPanic {
                renderer: format!("{:?}", MarkupLanguage::ReStructuredText),
                msg: String::new(),
            }
        );
        let err_msg = result.unwrap_err().to_string();
        assert!(
            err_msg.starts_with(&expected_prefix),
            "expected NoteError::RenderPanic prefix {expected_prefix:?}, got: {err_msg}"
        );
    }

    #[test]
    fn test_incr_sort_tag_filter() {
        let ctx = tera::Context::new();
        let st = State::new(&ctx);

        let result =
            incr_sort_tag_filter(&Value::from("dir/19-Note.md"), Kwargs::default(), &st);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), Value::from("20"));

        let result = incr_sort_tag_filter(&Value::from("Note.md"), Kwargs::default(), &st);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), Value::from(""));

        let result = incr_sort_tag_filter(&Value::from("29-Note.md"), Kwargs::default(), &st);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), Value::from("30"));

        let result = incr_sort_tag_filter(&Value::from("02-Note.md"), Kwargs::default(), &st);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), Value::from("03"));

        let result = incr_sort_tag_filter(&Value::from("cz-Note.md"), Kwargs::default(), &st);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), Value::from("da"));

        let result =
            incr_sort_tag_filter(&Value::from("2cz-Note.md"), Kwargs::default(), &st);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), Value::from("2da"));

        // Too many letters, default string is ``.
        let result =
            incr_sort_tag_filter(&Value::from("2acz-Note.md"), Kwargs::default(), &st);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), Value::from(""));

        // No input.
        let result = incr_sort_tag_filter(
            &Value::from("-Note.md"),
            Kwargs::from([("default", Value::from("my default.md"))]),
            &st,
        );
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), Value::from("my default.md"));

        // Too big.
        let result = incr_sort_tag_filter(
            &Value::from("10000000000-Note.md"),
            Kwargs::from([("default", Value::from("my default.md"))]),
            &st,
        );
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), Value::from("my default.md"));

        // Too many digits.
        let result = incr_sort_tag_filter(
            &Value::from("013-Note.md"),
            Kwargs::from([("default", Value::from("my default.md"))]),
            &st,
        );
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), Value::from("014"));

        // Too big.
        let result = incr_sort_tag_filter(
            &Value::from("aaafbaz-Note.md"),
            Kwargs::from([("default", Value::from("my default.md"))]),
            &st,
        );
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), Value::from("my default.md"));

        // Too many digits.
        let result = incr_sort_tag_filter(
            &Value::from("aaf-Note.md"),
            Kwargs::from([("default", Value::from("my default.md"))]),
            &st,
        );
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), Value::from("my default.md"));

        let result = incr_sort_tag_filter(
            &Value::from("23-01-23-Note.md"),
            Kwargs::from([("default", Value::from("my default.md"))]),
            &st,
        );
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), Value::from("23-01-24"));
    }

    #[test]
    fn test_sanit_filter() {
        let ctx = tera::Context::new();
        let st = State::new(&ctx);

        let result = sanit_filter(
            &Value::from(".# Strange filename? Yes."),
            Kwargs::default(),
            &st,
        );
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), Value::from("Strange filename_ Yes"));

        let result =
            sanit_filter(&Value::from("Correct filename.pdf"), Kwargs::default(), &st);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), Value::from("Correct filename.pdf"));

        let result = sanit_filter(&Value::from(".dotfilename"), Kwargs::default(), &st);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), Value::from(".dotfilename"));
    }

    #[test]
    fn test_remove_filter() {
        let ctx = tera::Context::new();
        let st = State::new(&ctx);

        //
        let input =
            Value::from_serializable(&json!({"title": "my title", "subtitle": "my subtitle"}));
        let expected = Value::from_serializable(&json!({"subtitle": "my subtitle"}));
        let result =
            remove_filter(&input, Kwargs::from([("key", Value::from("title"))]), &st);
        assert_eq!(result.unwrap(), expected);

        //
        let input =
            Value::from_serializable(&json!({"title": "my title", "subtitle": "my subtitle"}));
        let expected =
            Value::from_serializable(&json!({"title": "my title", "subtitle": "my subtitle"}));
        let result = remove_filter(&input, Kwargs::from([("key", Value::from("nono"))]), &st);
        assert_eq!(result.unwrap(), expected);
    }

    #[test]
    fn test_insert_filter() {
        let ctx = tera::Context::new();
        let st = State::new(&ctx);

        //
        let input = Value::from_serializable(&json!({"subtitle": "my subtitle"}));
        let expected =
            Value::from_serializable(&json!({"new": "my new", "subtitle": "my subtitle"}));
        let result = insert_filter(
            &input,
            Kwargs::from([
                ("key", Value::from("fm_new")),
                ("value", Value::from("my new")),
            ]),
            &st,
        );
        assert_eq!(result.unwrap(), expected);

        //
        let input =
            Value::from_serializable(&json!({"title": "my title", "subtitle": "my subtitle"}));
        let expected = Value::from_serializable(
            &json!({"title": "my replaced title", "subtitle": "my subtitle"}),
        );
        let result = insert_filter(
            &input,
            Kwargs::from([
                ("key", Value::from("fm_title")),
                ("value", Value::from("my replaced title")),
            ]),
            &st,
        );
        assert_eq!(result.unwrap(), expected);

        //
        let input = Value::from_serializable(&json!({"title": "my title"}));
        let expected = Value::from_serializable(&json!({"new": null, "title": "my title"}));
        let result =
            insert_filter(&input, Kwargs::from([("key", Value::from("fm_new"))]), &st);
        assert_eq!(result.unwrap(), expected);
    }

    #[test]
    fn test_replace_emtpy_filter() {
        let ctx = tera::Context::new();
        let st = State::new(&ctx);

        // Do not replace.
        let result = replace_empty_filter(
            &Value::from("non empty string"),
            Kwargs::from([("with", Value::from("new string"))]),
            &st,
        );
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), Value::from("non empty string"));

        // Replace.
        let result = replace_empty_filter(
            &Value::from(""),
            Kwargs::from([("with", Value::from("new string"))]),
            &st,
        );
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), Value::from("new string"));

        // Array input, not empty.
        let input = Value::from_serializable(&json!([3, 4, 5]));
        let output = replace_empty_filter(
            &input,
            Kwargs::from([("with", Value::from_serializable(&json!([1, 2, 3])))]),
            &st,
        )
        .unwrap();
        assert_eq!(output, input);

        // Array input, empty.
        let input = Value::from_serializable(&json!([]));
        let output = replace_empty_filter(
            &input,
            Kwargs::from([("with", Value::from_serializable(&json!([1, 2, 3])))]),
            &st,
        )
        .unwrap();
        assert_eq!(output, Value::from_serializable(&json!([1, 2, 3])));

        // Array input, not empty.
        let input = Value::from_serializable(&json!(["", "not empty", ""]));
        let output = replace_empty_filter(
            &input,
            Kwargs::from([("with", Value::from_serializable(&json!([1, 2, 3])))]),
            &st,
        )
        .unwrap();
        assert_eq!(output, input);

        // Array input, empty.
        let input = Value::from_serializable(&json!(["", "", ""]));
        let output = replace_empty_filter(
            &input,
            Kwargs::from([("with", Value::from_serializable(&json!([1, 2, 3])))]),
            &st,
        )
        .unwrap();
        assert_eq!(output, Value::from_serializable(&json!([1, 2, 3])));

        // None input is treated as empty and replaced.
        let input = Value::none();
        let output = replace_empty_filter(
            &input,
            Kwargs::from([("with", Value::from_serializable(&json!([1, 2, 3])))]),
            &st,
        )
        .unwrap();
        assert_eq!(output, Value::from_serializable(&json!([1, 2, 3])));
    }

    #[test]
    fn test_prepend_filter() {
        let ctx = tera::Context::new();
        let st = State::new(&ctx);

        // `with`
        let result = prepend_filter(
            &Value::from("1. My first chapter"),
            Kwargs::from([("with", Value::from("-"))]),
            &st,
        );
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), Value::from("-1. My first chapter"));

        let result = prepend_filter(
            &Value::from(""),
            Kwargs::from([("with", Value::from("_"))]),
            &st,
        );
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), Value::from(""));

        // `with_sort_tag`
        let result = prepend_filter(
            &Value::from("1. My first chapter"),
            Kwargs::from([("with_sort_tag", Value::from("20230809"))]),
            &st,
        );
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), Value::from("20230809-1. My first chapter"));

        let result = prepend_filter(
            &Value::from("1-My first chapter"),
            Kwargs::from([("with_sort_tag", Value::from("20230809"))]),
            &st,
        );
        assert!(result.is_ok());
        assert_eq!(
            result.unwrap(),
            Value::from("20230809-'1-My first chapter")
        );

        let result = prepend_filter(
            &Value::from("1. My first chapter"),
            Kwargs::from([("with_sort_tag", Value::from(""))]),
            &st,
        );
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), Value::from("1. My first chapter"));

        let result = prepend_filter(
            &Value::from("1-My first chapter"),
            Kwargs::from([("with_sort_tag", Value::from(""))]),
            &st,
        );
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), Value::from("'1-My first chapter"));

        let result = prepend_filter(
            &Value::from(""),
            Kwargs::from([("with_sort_tag", Value::from("20230809"))]),
            &st,
        );
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), Value::from("20230809-'"));

        let result = prepend_filter(
            &Value::from(""),
            Kwargs::from([("with_sort_tag", Value::from(""))]),
            &st,
        );
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), Value::from("'"));

        // `with` + `newline`
        let result = prepend_filter(
            &Value::from("1. My first chapter"),
            Kwargs::from([("with", Value::from("-")), ("newline", Value::from(true))]),
            &st,
        );
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), Value::from("\n-1. My first chapter"));
    }

    #[test]
    fn test_append_filter() {
        let ctx = tera::Context::new();
        let st = State::new(&ctx);

        // `with`
        let result = append_filter(
            &Value::from("1. My first chapter"),
            Kwargs::from([("with", Value::from("-"))]),
            &st,
        );
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), Value::from("1. My first chapter-"));

        let result = append_filter(
            &Value::from(""),
            Kwargs::from([("with", Value::from("_"))]),
            &st,
        );
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), Value::from(""));

        // `with` + `newline`
        let result = append_filter(
            &Value::from("1. My first chapter"),
            Kwargs::from([("with", Value::from("-")), ("newline", Value::from(true))]),
            &st,
        );
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), Value::from("1. My first chapter-\n"));
    }

    #[test]
    fn test_link_text_link_dest_link_title_filter() {
        let ctx = tera::Context::new();
        let st = State::new(&ctx);

        // Test Markdown link in clipboard.
        let input = r#"xxx[Jens Getreu's blog](https://blog.getreu.net "My blog")"#;
        let output_ln =
            link_text_filter(&Value::from(input), Kwargs::default(), &st).unwrap();
        assert_eq!("Jens Getreu's blog", output_ln.as_str().unwrap());
        let output_lta =
            link_dest_filter(&Value::from(input), Kwargs::default(), &st).unwrap();
        assert_eq!("https://blog.getreu.net", output_lta.as_str().unwrap());
        let output_lti =
            link_title_filter(&Value::from(input), Kwargs::default(), &st).unwrap();
        assert_eq!("My blog", output_lti.as_str().unwrap());

        // Test non-link string in clipboard.
        let input = "Tp-Note helps you to quickly get\
            started writing notes.";
        let output_ln =
            link_text_filter(&Value::from(input), Kwargs::default(), &st).unwrap();
        assert_eq!("", output_ln.as_str().unwrap());
        let output_lta =
            link_dest_filter(&Value::from(input), Kwargs::default(), &st).unwrap();
        assert_eq!("", output_lta.as_str().unwrap());
        let output_lti =
            link_title_filter(&Value::from(input), Kwargs::default(), &st).unwrap();
        assert_eq!("", output_lti.as_str().unwrap());
    }

    #[test]
    fn test_link_text_filter() {
        let ctx = tera::Context::new();
        let st = State::new(&ctx);

        // Test Markdown link in clipboard.
        let input = r#"Some autolink: <tpnote:locallink.md>,
more autolinks: <tpnote:20>, <getreu@web.de>,
boring link text: [http://domain.com](http://getreu.net)
[Jens Getreu's blog](https://blog.getreu.net "My blog")
Some more text."#;

        let output = link_text_filter(&Value::from(input), Kwargs::default(), &st).unwrap();
        assert_eq!(output.as_str().unwrap(), "tpnote:locallink.md");

        // Test picky version also.
        let output =
            link_text_picky_filter(&Value::from(input), Kwargs::default(), &st).unwrap();
        assert_eq!(output.as_str().unwrap(), "Jens Getreu's blog");

        //
        let input = "[into\\_bytes](https://doc.rust-lang.org)";

        let output = link_text_filter(&Value::from(input), Kwargs::default(), &st).unwrap();
        assert_eq!(output.as_str().unwrap(), "into_bytes");

        // Test picky version also.
        let output =
            link_text_picky_filter(&Value::from(input), Kwargs::default(), &st).unwrap();
        assert_eq!(output.as_str().unwrap(), "into_bytes");
    }

    #[test]
    fn test_trunc_filter() {
        let ctx = tera::Context::new();
        let st = State::new(&ctx);

        let input = "Jens Getreu's blog";
        let output = trunc_filter(&Value::from(input), Kwargs::default(), &st).unwrap();
        assert_eq!("Jens Getr", output.as_str().unwrap());
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
        let ctx = tera::Context::new();
        let st = State::new(&ctx);

        //
        // Test find first sentence.
        let input = "N.ote.\nIt helps. Get quickly\
            started writing notes.";
        let output = heading_filter(&Value::from(input), Kwargs::default(), &st).unwrap();
        assert_eq!("N.ote", output.as_str().unwrap());

        //
        // Test find first sentence (Windows)
        let input = "N.ote.\r\nIt helps. Get quickly\
            started writing notes.";
        let output = heading_filter(&Value::from(input), Kwargs::default(), &st).unwrap();
        assert_eq!("N.ote", output.as_str().unwrap());

        //
        // Test find heading
        let input = "N.ote\n\nIt helps. Get quickly\
            started writing notes.";
        let output = heading_filter(&Value::from(input), Kwargs::default(), &st).unwrap();
        assert_eq!("N.ote", output.as_str().unwrap());

        //
        // Test find heading (Windows)
        let input = "N.ote\r\n\r\nIt helps. Get quickly\
            started writing notes.";
        let output = heading_filter(&Value::from(input), Kwargs::default(), &st).unwrap();
        assert_eq!("N.ote", output.as_str().unwrap());

        //
        // Test trim whitespace
        let input = "\r\n\r\n  \tIt helps. Get quickly\
            started writing notes.";
        let output = heading_filter(&Value::from(input), Kwargs::default(), &st).unwrap();
        assert_eq!("It helps", output.as_str().unwrap());
    }

    #[test]
    fn test_html_heading_filter() {
        let ctx = tera::Context::new();
        let st = State::new(&ctx);

        //
        // Test find first heading.
        let input = "Some text.<h1>Heading 1</h1>Get quickly\
            started writing notes.";
        let output = html_heading_filter(&Value::from(input), Kwargs::default(), &st).unwrap();
        assert_eq!("Heading 1", output.as_str().unwrap());

        //
        let input = "Some text.<h1 style=\"font-size:60px;\">\
            Heading 1</h1>Get quickly\
            started writing notes.";
        let output = html_heading_filter(&Value::from(input), Kwargs::default(), &st).unwrap();
        assert_eq!("Heading 1", output.as_str().unwrap());

        //
        let input = "Some text.<h2>Heading &amp;1</h2>Get quickly\
            started writing notes.";
        let output = html_heading_filter(&Value::from(input), Kwargs::default(), &st).unwrap();
        assert_eq!("Heading &1", output.as_str().unwrap());

        //
        let input = "Some text.<p>No Heading 1</p>Get quickly\
            started writing notes.";
        let output = html_heading_filter(&Value::from(input), Kwargs::default(), &st).unwrap();
        assert_eq!("", output.as_str().unwrap());

        //
        let input = "Some text.<h1>No Heading 1</p>Get quickly\
            started writing notes.";
        let output = html_heading_filter(&Value::from(input), Kwargs::default(), &st).unwrap();
        assert_eq!("", output.as_str().unwrap());

        //
        let input = "Some text.<p>No Heading 1</h1>Get quickly\
            started writing notes.";
        let output = html_heading_filter(&Value::from(input), Kwargs::default(), &st).unwrap();
        assert_eq!("", output.as_str().unwrap());

        //
        let input = "Some text.<p>No <h1>Heading 1</h1>Get quickly\
            started writing notes.";
        let output = html_heading_filter(&Value::from(input), Kwargs::default(), &st).unwrap();
        assert_eq!("Heading 1", output.as_str().unwrap());

        //
        let input = "Some text.<p>No <h1>Heading<br> 1</h1>Get quickly\
            started writing notes.";
        let output = html_heading_filter(&Value::from(input), Kwargs::default(), &st).unwrap();
        assert_eq!("Heading 1", output.as_str().unwrap());

        //
        let input = "<p>No <h1>Heading 1</h1> <h1>Heading 2</h1> text";
        let output = html_heading_filter(&Value::from(input), Kwargs::default(), &st).unwrap();
        assert_eq!("Heading 1", output.as_str().unwrap());
    }

    #[test]
    fn test_file_filter() {
        let ctx = tera::Context::new();
        let st = State::new(&ctx);

        //
        // Test file stem.
        let input =
            "/usr/local/WEB-SERVER-CONTENT/blog.getreu.net/projects/tp-note/20200908-My file.md";
        let output = file_stem_filter(&Value::from(input), Kwargs::default(), &st).unwrap();
        assert_eq!("My file", output.as_str().unwrap());

        let input =
            "/usr/local/WEB-SERVER-CONTENT/blog.getreu.net/projects/tp-note/20200908-My dir/";
        let output = file_stem_filter(&Value::from(input), Kwargs::default(), &st).unwrap();
        assert_eq!("My dir", output.as_str().unwrap());

        //
        // Test file tag.
        let input =
            "/usr/local/WEB-SERVER-CONTENT/blog.getreu.net/projects/tp-note/20200908-My file.md";
        let output =
            file_sort_tag_filter(&Value::from(input), Kwargs::default(), &st).unwrap();
        assert_eq!("20200908", output.as_str().unwrap());

        let input =
            "/usr/local/WEB-SERVER-CONTENT/blog.getreu.net/projects/tp-note/20200908-My dir/";
        let output =
            file_sort_tag_filter(&Value::from(input), Kwargs::default(), &st).unwrap();
        assert_eq!("20200908", output.as_str().unwrap());

        //
        // Test file extension.
        let input =
            "/usr/local/WEB-SERVER-CONTENT/blog.getreu.net/projects/tp-note/20200908-My file.md";
        let output = file_ext_filter(&Value::from(input), Kwargs::default(), &st).unwrap();
        assert_eq!("md", output.as_str().unwrap());

        let input = "/usr/local/WEB-SERVER-CONTENT/blog.getreu.net/projects/tp-note/20200908-My file.pfd.md";
        let output = file_ext_filter(&Value::from(input), Kwargs::default(), &st).unwrap();
        assert_eq!("md", output.as_str().unwrap());

        let input =
            "/usr/local/WEB-SERVER-CONTENT/blog.getreu.net/projects/tp-note/20200908-My dir/";
        let output = file_ext_filter(&Value::from(input), Kwargs::default(), &st).unwrap();
        assert_eq!("", output.as_str().unwrap());

        //
        // Test copy counter filter.
        let input = "/usr/local/WEB-SERVER-CONTENT/blog.getreu.net/projects/tp-note/20200908-My file(123).md";
        let output =
            file_copy_counter_filter(&Value::from(input), Kwargs::default(), &st).unwrap();
        assert_eq!(123, output.as_i64().unwrap());

        let input =
            "/usr/local/WEB-SERVER-CONTENT/blog.getreu.net/projects/tp-note/20200908-My dir/";
        let output = file_ext_filter(&Value::from(input), Kwargs::default(), &st).unwrap();
        assert_eq!("", output.as_str().unwrap());

        //
        // Test filename.
        let input = "/usr/local/WEB-SERVER-CONTENT/blog.getreu.net/projects/tp-note/20200908-My file(123).md";
        let output = file_name_filter(&Value::from(input), Kwargs::default(), &st).unwrap();
        assert_eq!("20200908-My file(123).md", output.as_str().unwrap());

        let input =
            "/usr/local/WEB-SERVER-CONTENT/blog.getreu.net/projects/tp-note/20200908-My dir/";
        let output = file_ext_filter(&Value::from(input), Kwargs::default(), &st).unwrap();
        assert_eq!("", output.as_str().unwrap());

        //
        // Test `prepend_dot`.
        let output = prepend_filter(
            &Value::from("md"),
            Kwargs::from([("with", Value::from("."))]),
            &st,
        )
        .unwrap();
        assert_eq!(".md", output.as_str().unwrap());

        let output = prepend_filter(
            &Value::from(""),
            Kwargs::from([("with", Value::from("."))]),
            &st,
        )
        .unwrap();
        assert_eq!("", output.as_str().unwrap());
    }

    #[test]
    fn test_map_lang_filter() {
        //
        // `Test `map_lang_filter()`
        use crate::settings::Settings;

        let ctx = tera::Context::new();
        let st = State::new(&ctx);

        let mut map_lang_filter_btmap = BTreeMap::new();
        map_lang_filter_btmap.insert("de".to_string(), "de-DE".to_string());
        let mut settings = SETTINGS.write();
        *settings = Settings::default();
        settings.map_lang_filter_btmap = Some(map_lang_filter_btmap);

        // This locks `SETTINGS` for further write access in this scope.
        let _settings = RwLockWriteGuard::<'_, _>::downgrade(settings);

        let input = Value::from_serializable(&json!(["de"]));
        let output = map_lang_filter(&input, Kwargs::default(), &st).unwrap();
        assert_eq!(Value::from_serializable(&json!(["de-DE"])), output);

        let input = Value::from_serializable(&json!(["de", "fr"]));
        let output = map_lang_filter(&input, Kwargs::default(), &st).unwrap();
        assert_eq!(Value::from_serializable(&json!(["de-DE", "fr"])), output);

        // None input is rejected by the type check.
        let input = Value::none();
        let result =
            map_lang_filter(&input, Kwargs::from([("default", Value::from("test"))]), &st);
        assert!(result.is_err());

        let input = Value::from_serializable(&json!([""]));
        let output = map_lang_filter(
            &input,
            Kwargs::from([("default", Value::from("test"))]),
            &st,
        )
        .unwrap();
        assert_eq!(Value::from_serializable(&json!(["test"])), output);

        let input = Value::from("this is not an array");
        let output = map_lang_filter(&input, Kwargs::default(), &st);
        assert!(output.is_err());

        let input = Value::from_serializable(&json!([3, 5, 8]));
        let output = map_lang_filter(&input, Kwargs::default(), &st);
        assert!(output.is_err());

        drop(_settings);
    }

    #[test]
    fn test_flatten_array_filter() {
        let ctx = tera::Context::new();
        let st = State::new(&ctx);

        // This is passed through.
        let input = Value::from_serializable(&json!(["de-DE", "fr", "et-ET"]));
        let output = flatten_array_filter(&input, Kwargs::default(), &st).unwrap();
        let arr = output.as_array().unwrap();
        assert_eq!("de-DE", arr[0].as_str().unwrap());
        assert_eq!("fr", arr[1].as_str().unwrap());
        assert_eq!("et-ET", arr[2].as_str().unwrap());

        // This input is rejected.
        let input = Value::from("de-DE");
        let output = flatten_array_filter(&input, Kwargs::default(), &st);
        assert!(output.is_err());

        // This is flattened.
        let input = Value::from_serializable(&json!(["de-DE"]));
        let output = flatten_array_filter(&input, Kwargs::default(), &st).unwrap();
        assert_eq!("de-DE", output.as_str().unwrap());
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
        // ReStructuredText link
        let input = "abc`Homepage <https://blog.getreu.net>`_\nabc";
        let expected_output = FirstHyperlink {
            text: "Homepage".into(),
            dest: "https://blog.getreu.net".into(),
            title: "".into(),
        };
        let output = FirstHyperlink::from(input);
        assert_eq!(expected_output, output.unwrap());

        //
        // ReStructuredText link ref
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
