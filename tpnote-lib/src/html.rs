//! Helper functions dealing with HTML conversion.
use crate::clone_ext::CloneExt;
use crate::error::InputStreamError;
use crate::filename::{NotePath, NotePathStr};
use crate::{
    config::{HeadingIdPolicy, LocalLinkKind},
    error::NoteError,
};
use html_escape;
use parking_lot::RwLock;
use parse_hyperlinks::parser::Link;
use parse_hyperlinks_extras::iterator_html::HtmlLinkInlineImage;
use percent_encoding::{AsciiSet, CONTROLS, percent_decode_str, utf8_percent_encode};
use std::path::MAIN_SEPARATOR_STR;
use std::{
    borrow::Cow,
    collections::HashSet,
    path::{Component, Path, PathBuf},
    sync::Arc,
};

pub(crate) const HTML_EXT: &str = ".html";

/// A local path can carry a format string at the end. This is the separator
/// character.
const FORMAT_SEPARATOR: char = '?';

/// If followed directly after FORMAT_SEPARATOR, it selects the sort-tag
/// for further matching.
const FORMAT_ONLY_SORT_TAG: char = '#';

/// If followed directly after FORMAT_SEPARATOR, it selects the whole filename
/// for further matching.
const FORMAT_COMPLETE_FILENAME: &str = "?";

/// A format string can be separated in a _from_ and _to_ part. This
/// optional separator is placed after `FORMAT_SEPARATOR` and separates
/// the _from_ and _to_ pattern.
const FORMAT_FROM_TO_SEPARATOR: char = ':';

/// Bytes that must be percent-encoded when a filesystem path segment is
/// embedded in an `href`/`src` attribute. `#` and `?` are URL syntax
/// (fragment and query introducers): left as-is, a literal one in a
/// directory or file name is read by the browser as the end of the path
/// and everything after it never reaches the server. `%` must be in this
/// set too, so `percent_encode_path()` escapes an existing `%` in a file
/// name before anything can be mistaken for one of its own escapes. The
/// space is encoded for consistency, even though browsers already encode
/// a literal space themselves before sending it.
static PATH_SEGMENT: &AsciiSet = &CONTROLS.add(b'#').add(b'?').add(b'%').add(b' ');

/// Splits `dest` into a filesystem path and a trailing URL fragment the
/// author wrote (`note.md#anchor`), mirroring the heuristic used
/// throughout this module: the last `#` starts a fragment only if it
/// falls in the final path segment, i.e. after the last `/` or `\`, or
/// there is no separator at all. A `#` that is part of a directory name
/// (`Meeting #12/notes.md`) precedes a later separator and is therefore
/// left in the path half. The returned fragment, if any, keeps its
/// leading `#`.
fn split_path_and_fragment(dest: &str) -> (&str, &str) {
    match (dest.rfind('#'), dest.rfind(['/', '\\'])) {
        (Some(n), sep) if sep.is_some_and(|sep| n > sep) || sep.is_none() => {
            (&dest[..n], &dest[n..])
        }
        _ => (dest, ""),
    }
}

/// Percent-encodes `path` for safe embedding in an `href`/`src` attribute,
/// segment by segment. Encoding is applied per segment, not to the joined
/// string, so the `/` separators — including a leading one — never need to
/// be exempted afterwards, which would risk exempting a `/` that was
/// actually part of a name. Bytes outside ASCII are always percent-encoded
/// by `utf8_percent_encode` as their UTF-8 octets.
fn percent_encode_path(path: &str) -> String {
    path.split('/')
        .map(|segment| utf8_percent_encode(segment, PATH_SEGMENT).to_string())
        .collect::<Vec<_>>()
        .join("/")
}

/// If `rewrite_rel_path` and `dest` is relative, concatenate `docdir` and
/// `dest`, then strip `root_path` from the left before returning.
/// If not `rewrite_rel_path` and `dest` is relative, return `dest`.
/// If `rewrite_abs_path` and `dest` is absolute, concatenate and return
/// `root_path` and `dest`.
/// If not `rewrite_abs_path` and `dest` is absolute, return `dest`.
/// The `dest` portion of the output is always canonicalized.
/// Return the assembled path, when in `root_path`, or `None` otherwise.
/// Asserts in debug mode, that `doc_dir` is in `root_path`.
fn assemble_link(
    root_path: &Path,
    docdir: &Path,
    dest: &Path,
    rewrite_rel_paths: bool,
    rewrite_abs_paths: bool,
) -> Option<PathBuf> {
    ///
    /// Concatenate `path` and `append`.
    /// The `append` portion of the output is if possible canonicalized.
    /// In case of underflow of an absolute link, the returned path is empty.
    fn append(path: &mut PathBuf, append: &Path) {
        // Append `dest` to `link` and canonicalize.
        for dir in append.components() {
            match dir {
                Component::ParentDir => {
                    if !path.pop() {
                        let path_is_relative = {
                            let mut c = path.components();
                            !(c.next() == Some(Component::RootDir)
                                || c.next() == Some(Component::RootDir))
                        };
                        if path_is_relative {
                            path.push(Component::ParentDir.as_os_str());
                        } else {
                            path.clear();
                            break;
                        }
                    }
                }
                Component::Normal(c) => path.push(c),
                _ => {}
            }
        }
    }

    // Under Windows `.is_relative()` does not detect `Component::RootDir`
    let dest_is_relative = {
        let mut c = dest.components();
        !(c.next() == Some(Component::RootDir) || c.next() == Some(Component::RootDir))
    };

    // Check if the link points into `root_path`, reject otherwise
    // (strip_prefix will not work).
    debug_assert!(docdir.starts_with(root_path));

    // Calculate the output.
    let mut link = match (rewrite_rel_paths, rewrite_abs_paths, dest_is_relative) {
        // *** Relative links.
        // Result: "/" + docdir.strip(root_path) + dest
        (true, false, true) => {
            let link = PathBuf::from(Component::RootDir.as_os_str());
            link.join(docdir.strip_prefix(root_path).ok()?)
        }
        // Result: docdir + dest
        (true, true, true) => docdir.to_path_buf(),
        // Result: dest
        (false, _, true) => PathBuf::new(),
        // *** Absolute links.
        // Result: "/" + dest
        (_, false, false) => PathBuf::from(Component::RootDir.as_os_str()),
        // Result: "/" + root_path
        (_, true, false) => root_path.to_path_buf(),
    };
    append(&mut link, dest);

    if link.as_os_str().is_empty() {
        None
    } else {
        Some(link)
    }
}

trait Hyperlink {
    /// A helper function, that first HTML escape decodes all strings of the
    /// link. Then it percent decodes the link destination (and the
    /// link text in case of an autolink).
    fn decode_ampersand_and_percent(&mut self);

    /// True if the value is a local link.
    #[allow(clippy::ptr_arg)]
    fn is_local_fn(value: &Cow<str>) -> bool;

    /// * `Link::Text2Dest`: strips a possible scheme in local `dest`.
    /// * `Link::Image2Dest`: strip local scheme in `dest`.
    /// * `Link::Image`: strip local scheme in `src`.
    ///
    ///  No action if not local.
    fn strip_local_scheme(&mut self);

    /// Helper function that strips a possible scheme in `input`.
    fn strip_scheme_fn(input: &mut Cow<str>);

    /// True if the link is:
    /// * `Link::Text2Dest` and the link text equals the link destination, or
    /// * `Link::Image` and the links `alt` equals the link source.
    ///
    /// WARNING: place this test after `decode_html_escape_and_percent()`
    /// and before: `rebase_local_link`, `expand_shorthand_link`,
    /// `rewrite_autolink` and `apply_format_attribute`.
    fn is_autolink(&self) -> bool;

    /// A method that converts the relative URLs (local links) in `self`.
    /// If successful, it returns `Ok(Some(URL))`, otherwise
    /// `Err(NoteError::InvalidLocalLink)`.
    /// If `self` contains an absolute URL, no conversion is performed and the
    /// return value is `Ok(())`.
    ///
    /// Conversion details:
    /// The base path for this conversion (usually where the HTML file resides),
    /// is `docdir`. If not `rewrite_rel_links`, relative local links are not
    /// converted. Furthermore, all local links starting with `/` are prepended
    /// with `root_path`. All absolute URLs always remain untouched.
    ///
    /// Algorithm:
    /// 1. If `rewrite_abs_links==true` and `link` starts with `/`, concatenate
    ///    and return `root_path` and `dest`.
    /// 2. If `rewrite_abs_links==false` and `dest` does not start wit `/`,
    ///    return `dest`.
    /// 3. If `rewrite_ext==true` and the link points to a known Tp-Note file
    ///    extension, then `.html` is appended to the converted link.
    ///
    /// Remark: The _anchor's text property_ is never changed. However, there
    /// is one exception: when the text contains a URL starting with `http:` or
    /// `https:`, only the file stem is kept. Example, the anchor text property:
    /// `<a ...>http:dir/my file.md</a>` is rewritten into `<a ...>my file</a>`.
    ///
    /// Contracts:
    /// 1. `link` may have a scheme.
    /// 2. `link` is `Link::Text2Dest` or `Link::Image`
    /// 3. `root_path` and `docdir` are absolute paths to directories.
    /// 4. `root_path` is never empty `""`. It can be `"/"`.
    fn rebase_local_link(
        &mut self,
        root_path: &Path,
        docdir: &Path,
        rewrite_rel_paths: bool,
        rewrite_abs_paths: bool,
    ) -> Result<(), NoteError>;

    /// If `dest` in `Link::Text2Dest` contains only a sort
    /// tag as filename, expand the latter to a full filename.
    /// Otherwise, no action.
    /// This method accesses the filesystem. Therefore sometimes `prepend_path`
    /// is needed as parameter and prepended.
    fn expand_shorthand_link(&mut self, prepend_path: Option<&Path>) -> Result<(), NoteError>;

    /// This removes a possible scheme in `text`.
    /// Call this method only when you sure that this
    /// is an autolink by testing with `is_autolink()`.
    fn rewrite_autolink(&mut self);

    /// A formatting attribute is a format string starting with `?` followed
    /// by one or two patterns. It is appended to `dest` or `src`.
    /// Processing details:
    /// 1. Extract some a possible formatting attribute string in `dest`
    ///    (`Link::Text2Dest`) or `src` (`Link::Image`) after `?`.
    /// 2. Extract the _path_ before `?` in `dest` or `src`.
    /// 3. Apply the formatting to _path_.
    /// 4. Store the result by overwriting `text` or `alt`.
    fn apply_format_attribute(&mut self);

    /// If the link destination `dest` is a local path, return it.
    /// Otherwise return `None`.
    /// Acts on `Link:Text2Dest` and `Link::Imgage2Dest` only.
    fn get_local_link_dest_path(&self) -> Option<&Path>;

    /// If `dest` or `src` is a local path, return it.
    /// Otherwise return `None`.
    /// Acts an `Link:Image` and `Link::Image2Dest` only.
    fn get_local_link_src_path(&self) -> Option<&Path>;

    /// If the extension of a local path in `dest` is some Tp-Note
    /// extension, append `.html` to the path. Otherwise silently return.
    /// Acts on `Link:Text2Dest` only.
    fn append_html_ext(&mut self);

    /// Renders `Link::Text2Dest`, `Link::Image2Dest` and `Link::Image`
    /// to HTML. Some characters in `dest` or `src` might be HTML
    /// escape encoded. This does not percent encode at all, because
    /// we know, that the result will be inserted later in a UTF-8 template.
    fn to_html(&self) -> String;
}

impl Hyperlink for Link<'_> {
    #[inline]
    fn decode_ampersand_and_percent(&mut self) {
        // HTML escape decode value.
        fn dec_amp(val: &mut Cow<str>) {
            let decoded_text = html_escape::decode_html_entities(val);
            if matches!(&decoded_text, Cow::Owned(..)) {
                // Does nothing, but satisfying the borrow checker. Does not `clone()`.
                let decoded_text = Cow::Owned(decoded_text.into_owned());
                // Store result.
                let _ = std::mem::replace(val, decoded_text);
            }
        }

        // HTML escape decode and percent decode value.
        fn dec_amp_percent(val: &mut Cow<str>) {
            dec_amp(val);
            let decoded_dest = percent_decode_str(val.as_ref()).decode_utf8().unwrap();
            if matches!(&decoded_dest, Cow::Owned(..)) {
                // Does nothing, but satisfying the borrow checker. Does not `clone()`.
                let decoded_dest = Cow::Owned(decoded_dest.into_owned());
                // Store result.
                let _ = std::mem::replace(val, decoded_dest);
            }
        }

        match self {
            Link::Text2Dest(text1, dest, title) => {
                dec_amp(text1);
                dec_amp_percent(dest);
                dec_amp(title);
            }
            Link::Image(alt, src) => {
                dec_amp(alt);
                dec_amp_percent(src);
            }
            Link::Image2Dest(text1, alt, src, text2, dest, title) => {
                dec_amp(text1);
                dec_amp(alt);
                dec_amp_percent(src);
                dec_amp(text2);
                dec_amp_percent(dest);
                dec_amp(title);
            }
            _ => unimplemented!(),
        };
    }

    //
    fn is_local_fn(dest: &Cow<str>) -> bool {
        !((dest.contains("://") && !dest.contains(":///"))
            || dest.starts_with("mailto:")
            || dest.starts_with("tel:"))
    }

    //
    fn strip_local_scheme(&mut self) {
        fn strip(dest: &mut Cow<str>) {
            if <Link<'_> as Hyperlink>::is_local_fn(dest) {
                <Link<'_> as Hyperlink>::strip_scheme_fn(dest);
            }
        }

        match self {
            Link::Text2Dest(_, dest, _title) => strip(dest),
            Link::Image2Dest(_, _, src, _, dest, _) => {
                strip(src);
                strip(dest);
            }
            Link::Image(_, src) => strip(src),
            _ => {}
        };
    }

    //
    fn strip_scheme_fn(inout: &mut Cow<str>) {
        let output = inout
            .trim_start_matches("https://")
            .trim_start_matches("https:")
            .trim_start_matches("http://")
            .trim_start_matches("http:")
            .trim_start_matches("tpnote:")
            .trim_start_matches("mailto:")
            .trim_start_matches("tel:");
        if output != inout.as_ref() {
            let _ = std::mem::replace(inout, Cow::Owned(output.to_string()));
        }
    }

    //
    fn is_autolink(&self) -> bool {
        let (text, dest) = match self {
            Link::Text2Dest(text, dest, _title) => (text, dest),
            Link::Image(alt, source) => (alt, source),
            // `Link::Image2Dest` is never an autolink.
            _ => return false,
        };
        text == dest
    }

    //
    fn rebase_local_link(
        &mut self,
        root_path: &Path,
        docdir: &Path,
        rewrite_rel_paths: bool,
        rewrite_abs_paths: bool,
    ) -> Result<(), NoteError> {
        let do_rebase = |path: &mut Cow<str>| -> Result<(), NoteError> {
            if <Link as Hyperlink>::is_local_fn(path) {
                let (path_part, fragment) = split_path_and_fragment(path.as_ref());
                if path_part.is_empty() {
                    // A bare fragment (`#ch1`) denotes the current document.
                    // There is no path to rebase.
                    return Ok(());
                }

                let dest_out = assemble_link(
                    root_path,
                    docdir,
                    Path::new(path_part),
                    rewrite_rel_paths,
                    rewrite_abs_paths,
                )
                .ok_or(NoteError::InvalidLocalPath {
                    path: path.as_ref().to_string(),
                })?;

                // Store result.
                let mut new_dest = dest_out.to_str().unwrap_or_default().to_string();
                new_dest.push_str(fragment);
                let _ = std::mem::replace(path, Cow::Owned(new_dest));
            }
            Ok(())
        };

        match self {
            Link::Text2Dest(_, dest, _) => do_rebase(dest),
            Link::Image2Dest(_, _, src, _, dest, _) => do_rebase(src).and_then(|_| do_rebase(dest)),
            Link::Image(_, src) => do_rebase(src),
            _ => unimplemented!(),
        }
    }

    //
    fn expand_shorthand_link(&mut self, prepend_path: Option<&Path>) -> Result<(), NoteError> {
        let shorthand_link = match self {
            Link::Text2Dest(_, dest, _) => dest,
            Link::Image2Dest(_, _, _, _, dest, _) => dest,
            _ => return Ok(()),
        };

        if !<Link as Hyperlink>::is_local_fn(shorthand_link) {
            return Ok(());
        }

        let (shorthand_str, shorthand_format) = match shorthand_link.split_once(FORMAT_SEPARATOR) {
            Some((path, fmt)) => (path, Some(fmt)),
            None => (shorthand_link.as_ref(), None),
        };

        let shorthand_path = Path::new(shorthand_str);

        if let Some(sort_tag) = shorthand_str.is_valid_sort_tag() {
            let full_shorthand_path = if let Some(root_path) = prepend_path {
                // Concatenate `root_path` and `shorthand_path`.
                let shorthand_path = shorthand_path
                    .strip_prefix(MAIN_SEPARATOR_STR)
                    .unwrap_or(shorthand_path);
                Cow::Owned(root_path.join(shorthand_path))
            } else {
                Cow::Borrowed(shorthand_path)
            };

            // Search for the file.
            let found = full_shorthand_path
                .parent()
                .and_then(|dir| dir.find_file_with_sort_tag(sort_tag));

            if let Some(path) = found {
                // We prepended `root_path` before, we can safely strip it
                // and unwrap.
                let found_link = path
                    .strip_prefix(prepend_path.unwrap_or(Path::new("")))
                    .unwrap();
                // Prepend `/`.
                let mut found_link = Path::new(MAIN_SEPARATOR_STR)
                    .join(found_link)
                    .to_str()
                    .unwrap_or_default()
                    .to_string();

                if let Some(fmt) = shorthand_format {
                    found_link.push(FORMAT_SEPARATOR);
                    found_link.push_str(fmt);
                }

                // Store result.
                let _ = std::mem::replace(shorthand_link, Cow::Owned(found_link));
            } else {
                return Err(NoteError::CanNotExpandShorthandLink {
                    path: full_shorthand_path.to_string_lossy().into_owned(),
                });
            }
        }
        Ok(())
    }

    //
    fn rewrite_autolink(&mut self) {
        let text = match self {
            Link::Text2Dest(text, _, _) => text,
            Link::Image(alt, _) => alt,
            _ => return,
        };

        <Link as Hyperlink>::strip_scheme_fn(text);
    }

    //
    fn apply_format_attribute(&mut self) {
        // Is this an absolute URL?

        let (text, dest) = match self {
            Link::Text2Dest(text, dest, _) => (text, dest),
            Link::Image(alt, source) => (alt, source),
            _ => return,
        };

        if !<Link as Hyperlink>::is_local_fn(dest) {
            return;
        }

        // We assume, that `dest` had been expanded already, so we can extract
        // the full filename here.
        // If ever it ends with a format string we apply it. Otherwise we quit
        // the method and do nothing.
        let (path, format) = match dest.split_once(FORMAT_SEPARATOR) {
            Some(s) => s,
            None => return,
        };

        let mut short_text = Path::new(path)
            .file_name()
            .unwrap_or_default()
            .to_str()
            .unwrap_or_default();

        // Select what to match:
        let format = if format.starts_with(FORMAT_COMPLETE_FILENAME) {
            // Keep complete filename.
            format
                .strip_prefix(FORMAT_COMPLETE_FILENAME)
                .unwrap_or(format)
        } else if format.starts_with(FORMAT_ONLY_SORT_TAG) {
            // Keep only format-tag.
            short_text = Path::new(path).disassemble().0;
            format.strip_prefix(FORMAT_ONLY_SORT_TAG).unwrap_or(format)
        } else {
            // Keep only stem.
            short_text = Path::new(path).disassemble().2;
            format
        };

        match format.split_once(FORMAT_FROM_TO_SEPARATOR) {
            // No `:`
            None => {
                if !format.is_empty()
                    && let Some(idx) = short_text.find(format) {
                        short_text = &short_text[..idx];
                    };
            }
            // Some `:`
            Some((from, to)) => {
                if !from.is_empty()
                    && let Some(idx) = short_text.find(from) {
                        short_text = &short_text[(idx + from.len())..];
                    };
                if !to.is_empty()
                    && let Some(idx) = short_text.find(to) {
                        short_text = &short_text[..idx];
                    };
            }
        }
        // Store the result.
        let _ = std::mem::replace(text, Cow::Owned(short_text.to_string()));
        let _ = std::mem::replace(dest, Cow::Owned(path.to_string()));
    }

    //
    fn get_local_link_dest_path(&self) -> Option<&Path> {
        let dest = match self {
            Link::Text2Dest(_, dest, _) => dest,
            Link::Image2Dest(_, _, _, _, dest, _) => dest,
            _ => return None,
        };
        if <Link as Hyperlink>::is_local_fn(dest) {
            let path = split_path_and_fragment(dest.as_ref()).0;
            (!path.is_empty()).then(|| Path::new(path))
        } else {
            None
        }
    }

    //
    fn get_local_link_src_path(&self) -> Option<&Path> {
        let src = match self {
            Link::Image2Dest(_, _, src, _, _, _) => src,
            Link::Image(_, src) => src,
            _ => return None,
        };
        if <Link as Hyperlink>::is_local_fn(src) {
            Some(Path::new(src.as_ref()))
        } else {
            None
        }
    }

    //
    fn append_html_ext(&mut self) {
        let dest = match self {
            Link::Text2Dest(_, dest, _) => dest,
            Link::Image2Dest(_, _, _, _, dest, _) => dest,
            _ => return,
        };
        if <Link as Hyperlink>::is_local_fn(dest) {
            let (path, fragment) = split_path_and_fragment(dest.as_ref());
            if path.has_tpnote_ext() {
                let mut newpath = path.to_string();
                newpath.push_str(HTML_EXT);
                newpath.push_str(fragment);

                let _ = std::mem::replace(dest, Cow::Owned(newpath));
            }
        }
    }

    //
    fn to_html(&self) -> String {
        // HTML escape encode double quoted attributes
        fn enc_amp(val: Cow<str>) -> Cow<str> {
            let s = html_escape::encode_double_quoted_attribute(val.as_ref());
            if s == val {
                val
            } else {
                // No cloning happens here, because we own `s` already.
                Cow::Owned(s.into_owned())
            }
        }
        // Replace Windows backslash, percent-encode the path (keeping a
        // written fragment untouched), then HTML escape encode.
        fn repl_backspace_enc_amp(val: Cow<str>) -> Cow<str> {
            // Under Windows `\` is a path separator, not data: normalize it
            // to `/` before `split_path_and_fragment`/`percent_encode_path`
            // treat it as one.
            let val = if val.as_ref().contains('\\') {
                Cow::Owned(val.to_string().replace('\\', "/"))
            } else {
                val
            };
            let (path, fragment) = split_path_and_fragment(val.as_ref());
            let encoded = format!("{}{}", percent_encode_path(path), fragment);
            let s = html_escape::encode_double_quoted_attribute(&encoded);
            Cow::Owned(s.into_owned())
        }

        match self {
            Link::Text2Dest(text, dest, title) => {
                // Format title.
                let title_html = if !title.is_empty() {
                    format!(" title=\"{}\"", enc_amp(title.shallow_clone()))
                } else {
                    "".to_string()
                };

                format!(
                    "<a href=\"{}\"{}>{}</a>",
                    repl_backspace_enc_amp(dest.shallow_clone()),
                    title_html,
                    text
                )
            }
            Link::Image2Dest(text1, alt, src, text2, dest, title) => {
                // Format title.
                let title_html = if !title.is_empty() {
                    format!(" title=\"{}\"", enc_amp(title.shallow_clone()))
                } else {
                    "".to_string()
                };

                format!(
                    "<a href=\"{}\"{}>{}<img src=\"{}\" alt=\"{}\">{}</a>",
                    repl_backspace_enc_amp(dest.shallow_clone()),
                    title_html,
                    text1,
                    repl_backspace_enc_amp(src.shallow_clone()),
                    enc_amp(alt.shallow_clone()),
                    text2
                )
            }
            Link::Image(alt, src) => {
                format!(
                    "<img src=\"{}\" alt=\"{}\">",
                    repl_backspace_enc_amp(src.shallow_clone()),
                    enc_amp(alt.shallow_clone())
                )
            }
            _ => unimplemented!(),
        }
    }
}

#[inline]
/// A helper function that scans the input HTML document in `html_input` for
/// HTML hyperlinks. When it finds a relative URL (local link), it analyzes it's
/// path. Depending on the `local_link_kind` configuration, relative local
/// links are converted into absolute local links and eventually rebased.
///
/// In order to achieve this, the user must respect the following convention
/// concerning absolute local links in Tp-Note documents:
/// 1. When a document contains a local link with an absolute path (absolute
///    local link), the base of this path is considered to be the directory
///    where the marker file ‘.tpnote.toml’ resides (or ‘/’ in non exists). The
///    marker file directory is `root_path`.
/// 2. Furthermore, the parameter `docdir` contains the absolute path of the
///    directory of the currently processed HTML document. The user guarantees
///    that `docdir` is the base for all relative local links in the document.
///    Note: `docdir` must always start with `root_path`.
///
/// If `LocalLinkKind::Off`, relative local links are not converted.
/// If `LocalLinkKind::Short`, relative local links are converted into an
/// absolute local links with `root_path` as base directory.
/// If `LocalLinkKind::Long`, in addition to the above, the resulting absolute
/// local link is prepended with `root_path`.
///
/// If `rewrite_ext` is true and a local link points to a known
/// Tp-Note file extension, then `.html` is appended to the converted link.
///
/// Remark: The link's text property is never changed. However, there is
/// one exception: when the link's text contains a string similar to URLs,
/// starting with `http:` or `tpnote:`. In this case, the string is interpreted
/// as URL and only the stem of the filename is displayed, e.g.
/// `<a ...>http:dir/my file.md</a>` is replaced with `<a ...>my file</a>`.
///
/// Finally, before a converted local link is reinserted in the output HTML, a
/// copy of that link is kept in `allowed_local_links` for further bookkeeping.
///
/// NB: All absolute URLs (starting with a domain) always remain untouched.
///
/// NB2: It is guaranteed, that the resulting HTML document contains only local
/// links to other documents within `root_path`. Deviant links displayed as
/// `INVALID LOCAL LINK` and URL is discarded.
pub fn rewrite_links(
    html_input: String,
    root_path: &Path,
    docdir: &Path,
    local_link_kind: LocalLinkKind,
    rewrite_ext: bool,
    allowed_local_links: Arc<RwLock<HashSet<PathBuf>>>,
) -> String {
    let (rewrite_rel_paths, rewrite_abs_paths) = match local_link_kind {
        LocalLinkKind::Off => (false, false),
        LocalLinkKind::Short => (true, false),
        LocalLinkKind::Long => (true, true),
    };

    // Search for hyperlinks and inline images in the HTML rendition
    // of this note.
    let mut rest = &*html_input;
    let mut html_out = String::new();
    for ((skipped, _consumed, remaining), mut link) in HtmlLinkInlineImage::new(&html_input) {
        html_out.push_str(skipped);
        rest = remaining;

        // Check if `text` = `dest`.
        let mut link_is_autolink = link.is_autolink();

        // Percent decode link destination.
        link.decode_ampersand_and_percent();

        // Check again if `text` = `dest`.
        link_is_autolink = link_is_autolink || link.is_autolink();

        link.strip_local_scheme();

        // Rewrite the local link.
        match link
            .rebase_local_link(root_path, docdir, rewrite_rel_paths, rewrite_abs_paths)
            .and_then(|_| {
                link.expand_shorthand_link(
                    (matches!(local_link_kind, LocalLinkKind::Short)).then_some(root_path),
                )
            }) {
            Ok(()) => {}
            Err(e) => {
                let e = e.to_string();
                let e = html_escape::encode_text(&e);
                html_out.push_str(&format!("<i>{}</i>", e));
                continue;
            }
        };

        if link_is_autolink {
            link.rewrite_autolink();
        }

        link.apply_format_attribute();

        if let Some(dest_path) = link.get_local_link_dest_path() {
            allowed_local_links.write().insert(dest_path.to_path_buf());
        };
        if let Some(src_path) = link.get_local_link_src_path() {
            allowed_local_links.write().insert(src_path.to_path_buf());
        };

        if rewrite_ext {
            link.append_html_ext();
        }
        html_out.push_str(&link.to_html());
    }
    // Add the last `remaining`.
    html_out.push_str(rest);

    log::trace!(
        "Viewer: referenced allowed local files: {}",
        allowed_local_links
            .read_recursive()
            .iter()
            .map(|p| {
                let mut s = "\n    '".to_string();
                s.push_str(&p.display().to_string());
                s
            })
            .collect::<String>()
    );

    html_out
    // The `RwLockWriteGuard` is released here.
}

/// One `<h1>`-`<h6>` heading found while scanning rendered HTML, in
/// document order.
struct HeadingMatch {
    /// Byte offset of the opening tag's terminating `>`.
    tag_close: usize,
    /// The opening tag's `id="..."` attribute value, if it already has one.
    existing_id: Option<String>,
    /// Tag-stripped, entity-decoded text content of the heading.
    text: String,
}

/// Scans `html` for every heading, in document order. Mirrors the
/// tag-finding approach of `filter::FirstHtmlHeading` (which stops at the
/// first heading; this collects all of them) and additionally extracts a
/// pre-existing `id="..."` attribute value, if present. Headings never
/// nest — CommonMark's grammar and RST's section model both forbid it — so
/// a simple "next matching closing tag" scan is safe.
fn scan_headings(html: &str) -> Vec<HeadingMatch> {
    const OPENING: &[&str; 6] = &["<h1", "<h2", "<h3", "<h4", "<h5", "<h6"];
    const CLOSING: &[&str; 6] = &["</h1>", "</h2>", "</h3>", "</h4>", "</h5>", "</h6>"];

    let mut headings = Vec::new();
    let mut i = 0;
    while let Some(mut tag_start) = html[i..].find('<') {
        let Some(mut tag_end) = html[i + tag_start..].find('>') else {
            break;
        };
        tag_end += 1;
        // Move on if there is another opening bracket.
        if let Some(new_start) = html[i + tag_start + 1..i + tag_start + tag_end].rfind('<') {
            tag_start += new_start + 1;
            tag_end -= new_start + 1;
        }

        let tag_str = &html[i + tag_start..i + tag_start + tag_end];
        if !OPENING.iter().any(|&pat| tag_str.starts_with(pat)) {
            i += tag_start + tag_end;
            continue;
        }

        // Index right after the opening tag's `>`, and of the `>` itself.
        let heading_start = i + tag_start + tag_end;
        let tag_close = heading_start - 1;

        let existing_id = tag_str.find("id=\"").map(|p| {
            let rest = &tag_str[p + 4..];
            let end = rest.find('"').unwrap_or(rest.len());
            rest[..end].to_string()
        });

        // Find the matching closing tag.
        let mut k = heading_start;
        let mut heading_end = None;
        while let Some(mut cs) = html[k..].find('<') {
            let Some(mut ce) = html[k + cs..].find('>') else {
                break;
            };
            ce += 1;
            if let Some(new_start) = html[k + cs + 1..k + cs + ce].rfind('<') {
                cs += new_start + 1;
                ce -= new_start + 1;
            }
            if CLOSING.iter().any(|&pat| html[k + cs..k + cs + ce].starts_with(pat)) {
                heading_end = Some(k + cs);
                break;
            }
            k += cs + ce;
        }

        let Some(heading_end) = heading_end else {
            i = heading_start;
            continue;
        };

        // Remove HTML tags inside the heading, then decode entities.
        let mut cleaned = String::new();
        let mut inside_tag = false;
        for c in html[heading_start..heading_end].chars() {
            if c == '<' {
                inside_tag = true;
            } else if c == '>' {
                inside_tag = false;
            } else if !inside_tag {
                cleaned.push(c);
            }
        }
        let text = html_escape::decode_html_entities(&cleaned).into_owned();

        headings.push(HeadingMatch { tag_close, existing_id, text });

        i = heading_end;
    }
    headings
}

/// GitHub/GitLab-style slug (see `HeadingIdPolicy::Gfm`): lowercase, keep
/// only Unicode letters/digits/`-`/`_`/space, convert each remaining space
/// to a hyphen individually (two adjacent spaces become two adjacent
/// hyphens, not one collapsed hyphen — this is what turns an em dash
/// surrounded by spaces into a double hyphen once the dash itself is
/// dropped), then trim stray leading/trailing hyphens.
fn slugify_gfm(text: &str) -> String {
    let lower = text.to_lowercase();
    let filtered: String = lower
        .chars()
        .filter(|c| c.is_alphanumeric() || *c == '-' || *c == '_' || *c == ' ')
        .map(|c| if c == ' ' { '-' } else { c })
        .collect();
    filtered.trim_matches('-').to_string()
}

/// Pandoc's `auto_identifiers` algorithm (see `HeadingIdPolicy::Pandoc`):
/// like `slugify_gfm`, but periods are also kept, and any leading run of
/// non-letter characters is stripped (`2. Section` becomes `section`, not
/// `2-section`).
fn slugify_pandoc(text: &str) -> String {
    let lower = text.to_lowercase();
    let filtered: String = lower
        .chars()
        .filter(|c| c.is_alphanumeric() || *c == '-' || *c == '_' || *c == '.' || *c == ' ')
        .map(|c| if c == ' ' { '-' } else { c })
        .collect();
    filtered
        .trim_start_matches(|c: char| !c.is_alphabetic())
        .trim_matches('-')
        .to_string()
}

/// Appends `-1`, `-2`, ... to `base` until the result isn't already in
/// `seen`, records the result in `seen`, and returns it.
fn disambiguate(base: String, seen: &mut HashSet<String>) -> String {
    if seen.insert(base.clone()) {
        return base;
    }
    let mut n = 1;
    loop {
        let candidate = format!("{base}-{n}");
        if seen.insert(candidate.clone()) {
            return candidate;
        }
        n += 1;
    }
}

/// Assigns an `id` attribute to every heading in `html` that doesn't
/// already have one, according to `policy`. A heading's existing `id` —
/// whether an explicit Markdown `{#id}` heading attribute or one another
/// renderer already assigned (e.g. RST's own auto-ids) — always wins and is
/// never touched. Markup-language agnostic: this runs on the final
/// rendered HTML, after `markup_to_html` has already produced `<h1>`-`<h6>`
/// tags, regardless of which renderer produced them.
pub fn assign_heading_ids(html: String, policy: HeadingIdPolicy) -> String {
    if policy == HeadingIdPolicy::Off {
        return html;
    }

    let headings = scan_headings(&html);
    if headings.is_empty() {
        return html;
    }

    let mut seen: HashSet<String> = HashSet::new();
    for h in &headings {
        if let Some(id) = &h.existing_id {
            seen.insert(id.clone());
        }
    }

    // `(tag_close offset, new id)` for every heading that needs one, in
    // document order.
    let mut assignments: Vec<(usize, String)> = Vec::new();
    for h in &headings {
        if h.existing_id.is_some() {
            continue;
        }
        let slug = match policy {
            HeadingIdPolicy::Gfm => slugify_gfm(&h.text),
            HeadingIdPolicy::Pandoc => slugify_pandoc(&h.text),
            HeadingIdPolicy::Off => unreachable!(),
        };
        let slug = if slug.is_empty() {
            match policy {
                // Pandoc documents this fallback explicitly.
                HeadingIdPolicy::Pandoc => "section".to_string(),
                // No spec for this case in GFM/GitLab: leave the heading
                // id-less rather than fabricate one.
                _ => continue,
            }
        } else {
            slug
        };
        assignments.push((h.tag_close, disambiguate(slug, &mut seen)));
    }

    if assignments.is_empty() {
        return html;
    }

    // Splice `id="..."` into each opening tag right before its `>`. Offsets
    // are into the original `html`, in ascending order, so a running
    // cursor over spans of the original string is enough to rebuild it.
    let mut out = String::with_capacity(html.len() + assignments.len() * 16);
    let mut cursor = 0;
    for (tag_close, id) in assignments {
        out.push_str(&html[cursor..tag_close]);
        out.push_str(&format!(" id=\"{id}\""));
        cursor = tag_close;
    }
    out.push_str(&html[cursor..]);
    out
}

/// This trait deals with tagged HTML `&str` data.
pub trait HtmlStr {
    /// Lowercase pattern to check if this is a Doctype tag.
    const TAG_DOCTYPE_PAT: &'static str = "<!doctype";
    /// Lowercase pattern to check if this Doctype is HTML.
    const TAG_DOCTYPE_HTML_PAT: &'static str = "<!doctype html";
    /// Doctype HTML tag. This is inserted by
    /// `<HtmlString>.prepend_html_start_tag()`
    const TAG_DOCTYPE_HTML: &'static str = "<!DOCTYPE html>";
    /// Pattern to check if f this is an HTML start tag.
    const START_TAG_HTML_PAT: &'static str = "<html";
    /// HTML end tag.
    const END_TAG_HTML: &'static str = "</html>";

    /// We consider `self` empty, when it equals to `<!DOCTYPE html...>` or
    /// when it is empty.
    fn is_empty_html(&self) -> bool;

    /// We consider `html` empty, when it equals to `<!DOCTYPE html...>` or
    /// when it is empty.
    /// This is identical to `is_empty_html()`, but does not pull in
    /// additional trait bounds.
    fn is_empty_html2(html: &str) -> bool {
        html.is_empty_html()
    }

    /// True if stream starts with `<!DOCTYPE html...>`.
    fn has_html_start_tag(&self) -> bool;

    /// True if `html` starts with `<!DOCTYPE html...>`.
    /// This is identical to `has_html_start_tag()`, but does not pull in
    /// additional trait bounds.
    fn has_html_start_tag2(html: &str) -> bool {
        html.has_html_start_tag()
    }

    /// Some heuristics to guess if the input stream contains HTML.
    /// Current implementation:
    /// True if:
    ///
    /// * The stream starts with `<!DOCTYPE html ...>`, or
    /// * the stream starts with `<html ...>`    
    ///
    /// This function does not check if the recognized HTML is valid.
    fn is_html_unchecked(&self) -> bool;
}

impl HtmlStr for str {
    fn is_empty_html(&self) -> bool {
        if self.is_empty() {
            return true;
        }

        let html = self
            .trim_start()
            .lines()
            .next()
            .map(|l| l.to_ascii_lowercase())
            .unwrap_or_default();

        html.as_str().starts_with(Self::TAG_DOCTYPE_HTML_PAT)
            // The next closing bracket must be in last position.
            && html.find('>').unwrap_or_default() == html.len()-1
    }

    fn has_html_start_tag(&self) -> bool {
        let html = self
            .trim_start()
            .lines()
            .next()
            .map(|l| l.to_ascii_lowercase());
        html.as_ref()
            .is_some_and(|l| l.starts_with(Self::TAG_DOCTYPE_HTML_PAT))
    }

    fn is_html_unchecked(&self) -> bool {
        let html = self
            .trim_start()
            .lines()
            .next()
            .map(|l| l.to_ascii_lowercase());
        html.as_ref().is_some_and(|l| {
            (l.starts_with(Self::TAG_DOCTYPE_HTML_PAT)
                && l[Self::TAG_DOCTYPE_HTML_PAT.len()..].contains('>'))
                || (l.starts_with(Self::START_TAG_HTML_PAT)
                    && l[Self::START_TAG_HTML_PAT.len()..].contains('>'))
        })
    }
}

/// This trait deals with tagged HTML `String` data.
pub trait HtmlString: Sized {
    /// If the input does not start with `<!DOCTYPE html`
    /// (or lowercase variants), then insert `<!DOCTYPE html>`.
    /// Returns `InputStreamError::NonHtmlDoctype` if there is another Doctype
    /// already.
    fn prepend_html_start_tag(self) -> Result<Self, InputStreamError>;
}

impl HtmlString for String {
    fn prepend_html_start_tag(self) -> Result<Self, InputStreamError> {
        // Bring `HtmlStr` methods into scope.
        use crate::html::HtmlStr;

        let html2 = self
            .trim_start()
            .lines()
            .next()
            .map(|l| l.to_ascii_lowercase())
            .unwrap_or_default();

        if html2.starts_with(<str as HtmlStr>::TAG_DOCTYPE_HTML_PAT) {
            // Has a start tag already.
            Ok(self)
        } else if !html2.starts_with(<str as HtmlStr>::TAG_DOCTYPE_PAT) {
            // Insert HTML Doctype tag.
            let mut html = self;
            html.insert_str(0, <str as HtmlStr>::TAG_DOCTYPE_HTML);
            Ok(html)
        } else {
            // There is a Doctype other than HTML.
            Err(InputStreamError::NonHtmlDoctype {
                html: self.chars().take(25).collect::<String>(),
            })
        }
    }
}

#[cfg(test)]
mod tests {

    use crate::error::InputStreamError;
    use crate::error::NoteError;
    use crate::html::Hyperlink;
    use crate::html::assemble_link;
    use crate::html::assign_heading_ids;
    use crate::html::rewrite_links;
    use parking_lot::RwLock;
    use parse_hyperlinks::parser::Link;
    use parse_hyperlinks_extras::parser::parse_html::take_link;
    use std::borrow::Cow;
    use std::{
        collections::HashSet,
        path::{Path, PathBuf},
        sync::Arc,
    };

    #[test]
    fn test_assemble_link() {
        // `rewrite_rel_links=true`
        let output = assemble_link(
            Path::new("/my"),
            Path::new("/my/doc/path"),
            Path::new("../local/link to/note.md"),
            true,
            false,
        )
        .unwrap();
        assert_eq!(output, Path::new("/doc/local/link to/note.md"));

        // `rewrite_rel_links=false`
        let output = assemble_link(
            Path::new("/my"),
            Path::new("/my/doc/path"),
            Path::new("../local/link to/note.md"),
            false,
            false,
        )
        .unwrap();
        assert_eq!(output, Path::new("../local/link to/note.md"));

        // Absolute `dest`.
        let output = assemble_link(
            Path::new("/my"),
            Path::new("/my/doc/path"),
            Path::new("/test/../abs/local/link to/note.md"),
            false,
            false,
        )
        .unwrap();
        assert_eq!(output, Path::new("/abs/local/link to/note.md"));

        // Underflow.
        let output = assemble_link(
            Path::new("/my"),
            Path::new("/my/doc/path"),
            Path::new("/../local/link to/note.md"),
            false,
            false,
        );
        assert_eq!(output, None);

        // Absolute `dest`, `rewrite_abs_links=true`.
        let output = assemble_link(
            Path::new("/my"),
            Path::new("/my/doc/path"),
            Path::new("/abs/local/link to/note.md"),
            false,
            true,
        )
        .unwrap();
        assert_eq!(output, Path::new("/my/abs/local/link to/note.md"));

        // Absolute `dest`, `rewrite_abs_links=false`.
        let output = assemble_link(
            Path::new("/my"),
            Path::new("/my/doc/path"),
            Path::new("/test/../abs/local/link to/note.md"),
            false,
            false,
        )
        .unwrap();
        assert_eq!(output, Path::new("/abs/local/link to/note.md"));

        // Absolute `dest`, `rewrite` both.
        let output = assemble_link(
            Path::new("/my"),
            Path::new("/my/doc/path"),
            Path::new("abs/local/link to/note.md"),
            true,
            true,
        )
        .unwrap();
        assert_eq!(output, Path::new("/my/doc/path/abs/local/link to/note.md"));
    }

    #[test]
    fn test_decode_html_escape_and_percent() {
        //
        let mut input = Link::Text2Dest(Cow::from("text"), Cow::from("dest"), Cow::from("title"));
        let expected = Link::Text2Dest(Cow::from("text"), Cow::from("dest"), Cow::from("title"));
        input.decode_ampersand_and_percent();
        let output = input;
        assert_eq!(output, expected);

        //
        let mut input = Link::Text2Dest(
            Cow::from("te%20xt"),
            Cow::from("de%20st"),
            Cow::from("title"),
        );
        let expected =
            Link::Text2Dest(Cow::from("te%20xt"), Cow::from("de st"), Cow::from("title"));
        input.decode_ampersand_and_percent();
        let output = input;
        assert_eq!(output, expected);

        //
        let mut input =
            Link::Text2Dest(Cow::from("text"), Cow::from("d:e%20st"), Cow::from("title"));
        let expected = Link::Text2Dest(Cow::from("text"), Cow::from("d:e st"), Cow::from("title"));
        input.decode_ampersand_and_percent();
        let output = input;
        assert_eq!(output, expected);

        let mut input = Link::Text2Dest(
            Cow::from("a&amp;&quot;lt"),
            Cow::from("a&amp;&quot;lt"),
            Cow::from("a&amp;&quot;lt"),
        );
        let expected = Link::Text2Dest(
            Cow::from("a&\"lt"),
            Cow::from("a&\"lt"),
            Cow::from("a&\"lt"),
        );
        input.decode_ampersand_and_percent();
        let output = input;
        assert_eq!(output, expected);

        //
        let mut input = Link::Image(Cow::from("al%20t"), Cow::from("de%20st"));
        let expected = Link::Image(Cow::from("al%20t"), Cow::from("de st"));
        input.decode_ampersand_and_percent();
        let output = input;
        assert_eq!(output, expected);

        //
        let mut input = Link::Image(Cow::from("a\\lt"), Cow::from("d\\est"));
        let expected = Link::Image(Cow::from("a\\lt"), Cow::from("d\\est"));
        input.decode_ampersand_and_percent();
        let output = input;
        assert_eq!(output, expected);

        //
        let mut input = Link::Image(Cow::from("a&amp;&quot;lt"), Cow::from("a&amp;&quot;lt"));
        let expected = Link::Image(Cow::from("a&\"lt"), Cow::from("a&\"lt"));
        input.decode_ampersand_and_percent();
        let output = input;
        assert_eq!(output, expected);
    }

    #[test]
    fn test_is_local() {
        let input = Cow::from("/path/My doc.md");
        assert!(<Link as Hyperlink>::is_local_fn(&input));

        let input = Cow::from("tpnote:path/My doc.md");
        assert!(<Link as Hyperlink>::is_local_fn(&input));

        let input = Cow::from("tpnote:/path/My doc.md");
        assert!(<Link as Hyperlink>::is_local_fn(&input));

        let input = Cow::from("https://getreu.net");
        assert!(!<Link as Hyperlink>::is_local_fn(&input));
    }

    #[test]
    fn strip_local_scheme() {
        let mut input = Link::Text2Dest(
            Cow::from("xyz"),
            Cow::from("https://getreu.net"),
            Cow::from("xyz"),
        );
        let expected = input.clone();
        input.strip_local_scheme();
        assert_eq!(input, expected);

        //
        let mut input = Link::Text2Dest(
            Cow::from("xyz"),
            Cow::from("tpnote:/dir/My doc.md"),
            Cow::from("xyz"),
        );
        let expected = Link::Text2Dest(
            Cow::from("xyz"),
            Cow::from("/dir/My doc.md"),
            Cow::from("xyz"),
        );
        input.strip_local_scheme();
        assert_eq!(input, expected);
    }

    #[test]
    fn test_is_autolink() {
        let input = Link::Image(Cow::from("abc"), Cow::from("abc"));
        assert!(input.is_autolink());

        //
        let input = Link::Text2Dest(Cow::from("abc"), Cow::from("abc"), Cow::from("xyz"));
        assert!(input.is_autolink());

        //
        let input = Link::Image(Cow::from("abc"), Cow::from("abcd"));
        assert!(!input.is_autolink());

        //
        let input = Link::Text2Dest(Cow::from("abc"), Cow::from("abcd"), Cow::from("xyz"));
        assert!(!input.is_autolink());
    }

    #[test]
    fn test_rewrite_local_link() {
        let root_path = Path::new("/my/");
        let docdir = Path::new("/my/abs/note path/");

        // Should panic: this is not a relative path.
        let mut input = take_link("<a href=\"ftp://getreu.net\">Blog</a>")
            .unwrap()
            .1
            .1;
        input
            .rebase_local_link(root_path, docdir, true, false)
            .unwrap();
        assert!(input.get_local_link_dest_path().is_none());

        //
        let root_path = Path::new("/my/");
        let docdir = Path::new("/my/abs/note path/");

        // Check relative path to image.
        let mut input = take_link("<img src=\"down/./down/../../t m p.jpg\" alt=\"Image\" />")
            .unwrap()
            .1
            .1;
        let expected = "<img src=\"/abs/note%20path/t%20m%20p.jpg\" \
            alt=\"Image\">";
        input
            .rebase_local_link(root_path, docdir, true, false)
            .unwrap();
        let outpath = input.get_local_link_src_path().unwrap();
        let output = input.to_html();
        assert_eq!(output, expected);
        assert_eq!(outpath, PathBuf::from("/abs/note path/t m p.jpg"));

        // Check relative path to image. Canonicalized?
        let mut input = take_link("<img src=\"down/./../../t m p.jpg\" alt=\"Image\" />")
            .unwrap()
            .1
            .1;
        let expected = "<img src=\"/abs/t%20m%20p.jpg\" alt=\"Image\">";
        input
            .rebase_local_link(root_path, docdir, true, false)
            .unwrap();
        let outpath = input.get_local_link_src_path().unwrap();
        let output = input.to_html();
        assert_eq!(output, expected);
        assert_eq!(outpath, PathBuf::from("/abs/t m p.jpg"));

        // Check relative path to note file.
        let mut input = take_link("<a href=\"./down/./../my note 1.md\">my note 1</a>")
            .unwrap()
            .1
            .1;
        let expected = "<a href=\"/abs/note%20path/my%20note%201.md\">my note 1</a>";
        input
            .rebase_local_link(root_path, docdir, true, false)
            .unwrap();
        let outpath = input.get_local_link_dest_path().unwrap();
        let output = input.to_html();
        assert_eq!(output, expected);
        assert_eq!(outpath, PathBuf::from("/abs/note path/my note 1.md"));

        // Check absolute path to note file.
        let mut input = take_link("<a href=\"/dir/./down/../my note 1.md\">my note 1</a>")
            .unwrap()
            .1
            .1;
        let expected = "<a href=\"/dir/my%20note%201.md\">my note 1</a>";
        input
            .rebase_local_link(root_path, docdir, true, false)
            .unwrap();
        let outpath = input.get_local_link_dest_path().unwrap();
        let output = input.to_html();
        assert_eq!(output, expected);
        assert_eq!(outpath, PathBuf::from("/dir/my note 1.md"));

        // Check relative path to note file. Canonicalized?
        let mut input = take_link("<a href=\"./down/./../dir/my note 1.md\">my note 1</a>")
            .unwrap()
            .1
            .1;
        let expected = "<a href=\"dir/my%20note%201.md\">my note 1</a>";
        input
            .rebase_local_link(root_path, docdir, false, false)
            .unwrap();
        let outpath = input.get_local_link_dest_path().unwrap();
        let output = input.to_html();
        assert_eq!(output, expected);
        assert_eq!(outpath, PathBuf::from("dir/my note 1.md"));

        // Check relative link in input.
        let mut input = take_link("<a href=\"./down/./../dir/my note 1.md\">my note 1</a>")
            .unwrap()
            .1
            .1;
        let expected = "<a href=\"/path/dir/my%20note%201.md\">my note 1</a>";
        input
            .rebase_local_link(
                Path::new("/my/note/"),
                Path::new("/my/note/path/"),
                true,
                false,
            )
            .unwrap();
        let outpath = input.get_local_link_dest_path().unwrap();
        let output = input.to_html();
        assert_eq!(output, expected);
        assert_eq!(outpath, PathBuf::from("/path/dir/my note 1.md"));

        // Check absolute link in input.
        let mut input = take_link("<a href=\"/down/./../dir/my note 1.md\">my note 1</a>")
            .unwrap()
            .1
            .1;
        let expected = "<a href=\"/dir/my%20note%201.md\">my note 1</a>";
        input
            .rebase_local_link(root_path, Path::new("/my/ignored/"), true, false)
            .unwrap();
        let outpath = input.get_local_link_dest_path().unwrap();
        let output = input.to_html();
        assert_eq!(output, expected);
        assert_eq!(outpath, PathBuf::from("/dir/my note 1.md"));

        // Check absolute link in input, not in `root_path`.
        let mut input = take_link("<a href=\"/down/../../dir/my note 1.md\">my note 1</a>")
            .unwrap()
            .1
            .1;
        let output = input
            .rebase_local_link(root_path, Path::new("/my/notepath/"), true, false)
            .unwrap_err();
        assert!(matches!(output, NoteError::InvalidLocalPath { .. }));

        // Check relative link in input, not in `root_path`.
        let mut input = take_link("<a href=\"../../dir/my note 1.md\">my note 1</a>")
            .unwrap()
            .1
            .1;
        let output = input
            .rebase_local_link(root_path, Path::new("/my/notepath/"), true, false)
            .unwrap_err();
        assert!(matches!(output, NoteError::InvalidLocalPath { .. }));

        // Check relative link in input, with underflow.
        let root_path = Path::new("/");
        let mut input = take_link("<a href=\"../../dir/my note 1.md\">my note 1</a>")
            .unwrap()
            .1
            .1;
        let output = input
            .rebase_local_link(root_path, Path::new("/my/"), true, false)
            .unwrap_err();
        assert!(matches!(output, NoteError::InvalidLocalPath { .. }));

        // Check relative link in input, not in `root_path`.
        let root_path = Path::new("/my");
        let mut input = take_link("<a href=\"../../dir/my note 1.md\">my note 1</a>")
            .unwrap()
            .1
            .1;
        let output = input
            .rebase_local_link(root_path, Path::new("/my/notepath"), true, false)
            .unwrap_err();
        assert!(matches!(output, NoteError::InvalidLocalPath { .. }));

        // Test autolink.
        let root_path = Path::new("/my");
        let mut input =
            take_link("<a href=\"tpnote:dir/3.0-my note.md\">tpnote:dir/3.0-my note.md</a>")
                .unwrap()
                .1
                .1;
        input.strip_local_scheme();
        input
            .rebase_local_link(root_path, Path::new("/my/path"), true, false)
            .unwrap();
        input.rewrite_autolink();
        input.apply_format_attribute();
        let outpath = input.get_local_link_dest_path().unwrap();
        let output = input.to_html();
        let expected = "<a href=\"/path/dir/3.0-my%20note.md\">dir/3.0-my note.md</a>";
        assert_eq!(output, expected);
        assert_eq!(outpath, PathBuf::from("/path/dir/3.0-my note.md"));

        // Test short autolink 1 with sort-tag only.
        let root_path = Path::new("/my");
        let mut input = take_link("<a href=\"tpnote:dir/3.0\">tpnote:dir/3.0</a>")
            .unwrap()
            .1
            .1;
        input.strip_local_scheme();
        input
            .rebase_local_link(root_path, Path::new("/my/path"), true, false)
            .unwrap();
        input.rewrite_autolink();
        input.apply_format_attribute();
        let outpath = input.get_local_link_dest_path().unwrap();
        let output = input.to_html();
        let expected = "<a href=\"/path/dir/3.0\">dir/3.0</a>";
        assert_eq!(output, expected);
        assert_eq!(outpath, PathBuf::from("/path/dir/3.0"));

        // The link text contains inline content.
        let root_path = Path::new("/my");
        let mut input = take_link(
            "<a href=\
            \"/uri\">link <em>foo <strong>bar</strong> <code>#</code></em>\
            </a>",
        )
        .unwrap()
        .1
        .1;
        input.strip_local_scheme();
        input
            .rebase_local_link(root_path, Path::new("/my/path"), true, false)
            .unwrap();
        let outpath = input.get_local_link_dest_path().unwrap();
        let expected = "<a href=\"/uri\">link <em>foo <strong>bar\
            </strong> <code>#</code></em></a>";

        let output = input.to_html();
        assert_eq!(output, expected);
        assert_eq!(outpath, PathBuf::from("/uri"));
    }

    #[test]
    fn test_rewrite_autolink() {
        //
        let mut input = Link::Text2Dest(
            Cow::from("http://getreu.net"),
            Cow::from("http://getreu.net"),
            Cow::from("title"),
        );
        let expected = Link::Text2Dest(
            Cow::from("getreu.net"),
            Cow::from("http://getreu.net"),
            Cow::from("title"),
        );
        input.rewrite_autolink();
        let output = input;
        assert_eq!(output, expected);

        //
        let mut input = Link::Text2Dest(
            Cow::from("/dir/3.0"),
            Cow::from("/dir/3.0-My note.md"),
            Cow::from("title"),
        );
        let expected = Link::Text2Dest(
            Cow::from("/dir/3.0"),
            Cow::from("/dir/3.0-My note.md"),
            Cow::from("title"),
        );
        input.rewrite_autolink();
        let output = input;
        assert_eq!(output, expected);

        //
        let mut input = Link::Text2Dest(
            Cow::from("tpnote:/dir/3.0"),
            Cow::from("/dir/3.0-My note.md"),
            Cow::from("title"),
        );
        let expected = Link::Text2Dest(
            Cow::from("/dir/3.0"),
            Cow::from("/dir/3.0-My note.md"),
            Cow::from("title"),
        );
        input.rewrite_autolink();
        let output = input;
        assert_eq!(output, expected);

        //
        let mut input = Link::Text2Dest(
            Cow::from("tpnote:/dir/3.0"),
            Cow::from("/dir/3.0-My note.md?"),
            Cow::from("title"),
        );
        let expected = Link::Text2Dest(
            Cow::from("/dir/3.0"),
            Cow::from("/dir/3.0-My note.md?"),
            Cow::from("title"),
        );
        input.rewrite_autolink();
        let output = input;
        assert_eq!(output, expected);

        //
        let mut input = Link::Text2Dest(
            Cow::from("/dir/3.0-My note.md"),
            Cow::from("/dir/3.0-My note.md"),
            Cow::from("title"),
        );
        let expected = Link::Text2Dest(
            Cow::from("/dir/3.0-My note.md"),
            Cow::from("/dir/3.0-My note.md"),
            Cow::from("title"),
        );
        input.rewrite_autolink();
        let output = input;
        assert_eq!(output, expected);
    }

    #[test]
    fn test_apply_format_attribute() {
        //
        let mut input = Link::Text2Dest(
            Cow::from("tpnote:/dir/3.0"),
            Cow::from("/dir/3.0-My note.md"),
            Cow::from("title"),
        );
        let expected = Link::Text2Dest(
            Cow::from("tpnote:/dir/3.0"),
            Cow::from("/dir/3.0-My note.md"),
            Cow::from("title"),
        );
        input.apply_format_attribute();
        let output = input;
        assert_eq!(output, expected);

        //
        let mut input = Link::Text2Dest(
            Cow::from("does not matter"),
            Cow::from("/dir/3.0-My note.md?"),
            Cow::from("title"),
        );
        let expected = Link::Text2Dest(
            Cow::from("My note"),
            Cow::from("/dir/3.0-My note.md"),
            Cow::from("title"),
        );
        input.apply_format_attribute();
        let output = input;
        assert_eq!(output, expected);

        let mut input = Link::Text2Dest(
            Cow::from("/dir/3.0-My note--red_blue_green.jpg"),
            Cow::from("/dir/3.0-My note--red_blue_green.jpg"),
            Cow::from("title"),
        );
        let expected = Link::Text2Dest(
            Cow::from("/dir/3.0-My note--red_blue_green.jpg"),
            Cow::from("/dir/3.0-My note--red_blue_green.jpg"),
            Cow::from("title"),
        );
        input.apply_format_attribute();
        let output = input;
        assert_eq!(output, expected);

        //
        let mut input = Link::Text2Dest(
            Cow::from("does not matter"),
            Cow::from("/dir/3.0-My note--red_blue_green.jpg?"),
            Cow::from("title"),
        );
        let expected = Link::Text2Dest(
            Cow::from("My note--red_blue_green"),
            Cow::from("/dir/3.0-My note--red_blue_green.jpg"),
            Cow::from("title"),
        );
        input.apply_format_attribute();
        let output = input;
        assert_eq!(output, expected);

        //
        let mut input = Link::Text2Dest(
            Cow::from("does not matter"),
            Cow::from("/dir/3.0-My note--red_blue_green.jpg?--"),
            Cow::from("title"),
        );
        let expected = Link::Text2Dest(
            Cow::from("My note"),
            Cow::from("/dir/3.0-My note--red_blue_green.jpg"),
            Cow::from("title"),
        );
        input.apply_format_attribute();
        let output = input;
        assert_eq!(output, expected);

        //
        let mut input = Link::Text2Dest(
            Cow::from("does not matter"),
            Cow::from("/dir/3.0-My note--red_blue_green.jpg?_"),
            Cow::from("title"),
        );
        let expected = Link::Text2Dest(
            Cow::from("My note--red"),
            Cow::from("/dir/3.0-My note--red_blue_green.jpg"),
            Cow::from("title"),
        );
        input.apply_format_attribute();
        let output = input;
        assert_eq!(output, expected);

        //
        let mut input = Link::Text2Dest(
            Cow::from("does not matter"),
            Cow::from("/dir/3.0-My note--red_blue_green.jpg??"),
            Cow::from("title"),
        );
        let expected = Link::Text2Dest(
            Cow::from("3.0-My note--red_blue_green.jpg"),
            Cow::from("/dir/3.0-My note--red_blue_green.jpg"),
            Cow::from("title"),
        );
        input.apply_format_attribute();
        let output = input;
        assert_eq!(output, expected);

        //
        let mut input = Link::Text2Dest(
            Cow::from("does not matter"),
            Cow::from("/dir/3.0-My note--red_blue_green.jpg?#."),
            Cow::from("title"),
        );
        let expected = Link::Text2Dest(
            Cow::from("3"),
            Cow::from("/dir/3.0-My note--red_blue_green.jpg"),
            Cow::from("title"),
        );
        input.apply_format_attribute();
        let output = input;
        assert_eq!(output, expected);

        //
        let mut input = Link::Text2Dest(
            Cow::from("does not matter"),
            Cow::from("/dir/3.0-My note--red_blue_green.jpg??.:_"),
            Cow::from("title"),
        );
        let expected = Link::Text2Dest(
            Cow::from("0-My note--red"),
            Cow::from("/dir/3.0-My note--red_blue_green.jpg"),
            Cow::from("title"),
        );
        input.apply_format_attribute();
        let output = input;
        assert_eq!(output, expected);

        //
        let mut input = Link::Text2Dest(
            Cow::from("does not matter"),
            Cow::from("/dir/3.0-My note--red_blue_green.jpg?_:_"),
            Cow::from("title"),
        );
        let expected = Link::Text2Dest(
            Cow::from("blue"),
            Cow::from("/dir/3.0-My note--red_blue_green.jpg"),
            Cow::from("title"),
        );
        input.apply_format_attribute();
        let output = input;
        assert_eq!(output, expected);
    }

    #[test]
    fn get_local_link_dest_path() {
        //
        let input = Link::Text2Dest(Cow::from("xyz"), Cow::from("/dir/3.0"), Cow::from("title"));
        assert_eq!(
            input.get_local_link_dest_path(),
            Some(Path::new("/dir/3.0"))
        );

        //
        let input = Link::Text2Dest(
            Cow::from("xyz"),
            Cow::from("http://getreu.net"),
            Cow::from("title"),
        );
        assert_eq!(input.get_local_link_dest_path(), None);

        //
        let input = Link::Text2Dest(Cow::from("xyz"), Cow::from("dir/doc.md"), Cow::from("xyz"));
        let expected = Path::new("dir/doc.md");
        let res = input.get_local_link_dest_path().unwrap();
        assert_eq!(res, expected);

        //
        let input = Link::Text2Dest(Cow::from("xyz"), Cow::from("d#ir/doc.md"), Cow::from("xyz"));
        let expected = Path::new("d#ir/doc.md");
        let res = input.get_local_link_dest_path().unwrap();
        assert_eq!(res, expected);

        //
        let input = Link::Text2Dest(
            Cow::from("xyz"),
            Cow::from("dir/doc.md#1"),
            Cow::from("xyz"),
        );
        let expected = Path::new("dir/doc.md");
        let res = input.get_local_link_dest_path().unwrap();
        assert_eq!(res, expected);
    }

    #[test]
    fn test_split_path_and_fragment() {
        use crate::html::split_path_and_fragment;

        // A `#` in a directory name is data, not a fragment: it precedes a
        // later separator, so it stays in the path half.
        assert_eq!(
            split_path_and_fragment("Task #7/note.md"),
            ("Task #7/note.md", "")
        );

        // A `#` in the final segment, with no separator after it, is the
        // author's fragment.
        assert_eq!(
            split_path_and_fragment("note.md#anchor"),
            ("note.md", "#anchor")
        );

        // Both at once: exactly one `#` survives as a fragment, the one
        // the author wrote.
        assert_eq!(
            split_path_and_fragment("Task #7/note.md#anchor"),
            ("Task #7/note.md", "#anchor")
        );

        // No `#` at all.
        assert_eq!(split_path_and_fragment("dir/note.md"), ("dir/note.md", ""));

        // A bare fragment, no path.
        assert_eq!(split_path_and_fragment("#1"), ("", "#1"));
    }

    #[test]
    fn test_percent_encode_path() {
        use crate::html::percent_encode_path;
        use percent_encoding::percent_decode_str;

        // Round-trip: decoding what we encode returns the original bytes.
        for segment in [
            "Meeting #12-x",
            "a?b",
            "100%",
            "report %23.md",
            "with space",
            "a+b",
            "a&b",
            "em—dash",
            "a↔b",
            "already%20encoded",
        ] {
            let path = format!("/dir/{segment}/note.md");
            let encoded = percent_encode_path(&path);
            let decoded = percent_decode_str(&encoded).decode_utf8().unwrap();
            assert_eq!(decoded, path, "round-trip failed for segment {segment:?}");
        }

        // `#` and `?` are encoded so a browser cannot mistake them for URL
        // syntax.
        assert_eq!(
            percent_encode_path("/Meeting #12/note.md"),
            "/Meeting%20%2312/note.md"
        );
        assert_eq!(percent_encode_path("/a?b"), "/a%3Fb");

        // `%` is encoded first (and only once): a literal `%23` in a file
        // name must not be reinterpreted as an encoded `#`.
        assert_eq!(percent_encode_path("/report %23.md"), "/report%20%2523.md");
        let encoded = percent_encode_path("/report %23.md");
        let decoded = percent_decode_str(&encoded).decode_utf8().unwrap();
        assert_eq!(decoded, "/report %23.md");

        // The leading `/` and the `/` separators are never encoded.
        assert!(percent_encode_path("/a/b/c").starts_with('/'));
        assert_eq!(percent_encode_path("/a/b/c"), "/a/b/c");
    }

    #[test]
    fn test_append_html_ext() {
        //
        let mut input = Link::Text2Dest(
            Cow::from("abc"),
            Cow::from("/dir/3.0-My note.md"),
            Cow::from("title"),
        );
        let expected = Link::Text2Dest(
            Cow::from("abc"),
            Cow::from("/dir/3.0-My note.md.html"),
            Cow::from("title"),
        );
        input.append_html_ext();
        let output = input;
        assert_eq!(output, expected);
    }

    #[test]
    fn test_append_html_ext_with_fragment() {
        // The fragment must survive, reattached after the appended `.html`.
        let mut input = Link::Text2Dest(
            Cow::from("abc"),
            Cow::from("/dir/3.0-My note.md#ch1"),
            Cow::from("title"),
        );
        let expected = Link::Text2Dest(
            Cow::from("abc"),
            Cow::from("/dir/3.0-My note.md.html#ch1"),
            Cow::from("title"),
        );
        input.append_html_ext();
        let output = input;
        assert_eq!(output, expected);
    }

    #[test]
    fn test_to_html() {
        //
        let input = Link::Text2Dest(
            Cow::from("te\\x/t"),
            Cow::from("de\\s/t"),
            Cow::from("ti\\t/le"),
        );
        let expected = "<a href=\"de/s/t\" title=\"ti\\t/le\">te\\x/t</a>";
        let output = input.to_html();
        assert_eq!(output, expected);

        //
        let input = Link::Text2Dest(
            Cow::from("te&> xt"),
            Cow::from("de&> st"),
            Cow::from("ti&> tle"),
        );
        let expected = "<a href=\"de&amp;&gt;%20st\" title=\"ti&amp;&gt; tle\">te&> xt</a>";
        let output = input.to_html();
        assert_eq!(output, expected);

        //
        let input = Link::Image(Cow::from("al&t"), Cow::from("sr&c"));
        let expected = "<img src=\"sr&amp;c\" alt=\"al&amp;t\">";
        let output = input.to_html();
        assert_eq!(output, expected);

        //
        let input = Link::Text2Dest(Cow::from("te&> xt"), Cow::from("de&> st"), Cow::from(""));
        let expected = "<a href=\"de&amp;&gt;%20st\">te&> xt</a>";
        let output = input.to_html();
        assert_eq!(output, expected);
    }

    #[test]
    fn test_rewrite_links() {
        use crate::config::LocalLinkKind;

        let allowed_urls = Arc::new(RwLock::new(HashSet::new()));
        let input = "abc<a href=\"ftp://getreu.net\">Blog</a>\
            def<a href=\"https://getreu.net\">https://getreu.net</a>\
            ghi<img src=\"t m p.jpg\" alt=\"test 1\" />\
            jkl<a href=\"down/../down/my note 1.md\">my note 1</a>\
            mno<a href=\"http:./down/../dir/my note.md\">http:./down/../dir/my note.md</a>\
            pqr<a href=\"http:/down/../dir/my note.md\">\
            http:/down/../dir/my note.md</a>\
            stu<a href=\"http:/../dir/underflow/my note.md\">\
            not allowed dir</a>\
            vwx<a href=\"http:../../../not allowed dir/my note.md\">\
            not allowed</a>"
            .to_string();
        let expected = "abc<a href=\"ftp://getreu.net\">Blog</a>\
            def<a href=\"https://getreu.net\">getreu.net</a>\
            ghi<img src=\"/abs/note%20path/t%20m%20p.jpg\" alt=\"test 1\">\
            jkl<a href=\"/abs/note%20path/down/my%20note%201.md\">my note 1</a>\
            mno<a href=\"/abs/note%20path/dir/my%20note.md\">./down/../dir/my note.md</a>\
            pqr<a href=\"/dir/my%20note.md\">/down/../dir/my note.md</a>\
            stu<i>&lt;INVALID: /../dir/underflow/my note.md&gt;</i>\
            vwx<i>&lt;INVALID: ../../../not allowed dir/my note.md&gt;</i>"
            .to_string();

        let root_path = Path::new("/my/");
        let docdir = Path::new("/my/abs/note path/");
        let output = rewrite_links(
            input,
            root_path,
            docdir,
            LocalLinkKind::Short,
            false,
            allowed_urls.clone(),
        );
        let url = allowed_urls.read_recursive();

        assert!(url.contains(&PathBuf::from("/abs/note path/t m p.jpg")));
        assert!(url.contains(&PathBuf::from("/abs/note path/dir/my note.md")));
        assert!(url.contains(&PathBuf::from("/abs/note path/down/my note 1.md")));
        assert_eq!(output, expected);
    }

    #[test]
    fn test_rewrite_links2() {
        use crate::config::LocalLinkKind;

        let allowed_urls = Arc::new(RwLock::new(HashSet::new()));
        let input = "abd<a href=\"tpnote:dir/my note.md\">\
            <img src=\"/imagedir/favicon-32x32.png\" alt=\"logo\"></a>abd"
            .to_string();
        let expected = "abd<a href=\"/abs/note%20path/dir/my%20note.md\">\
            <img src=\"/imagedir/favicon-32x32.png\" alt=\"logo\"></a>abd";
        let root_path = Path::new("/my/");
        let docdir = Path::new("/my/abs/note path/");
        let output = rewrite_links(
            input,
            root_path,
            docdir,
            LocalLinkKind::Short,
            false,
            allowed_urls.clone(),
        );
        let url = allowed_urls.read_recursive();
        println!("{:?}", allowed_urls.read_recursive());
        assert!(url.contains(&PathBuf::from("/abs/note path/dir/my note.md")));
        assert_eq!(output, expected);
    }

    #[test]
    fn test_rewrite_links3() {
        use crate::config::LocalLinkKind;

        // A bare fragment (`#1`) denotes the current document: there is no
        // path to rebase, so it must survive every rewriting mode verbatim,
        // and it must not register the docdir as an "allowed" local link,
        // since no separate resource is referenced.
        let allowed_urls = Arc::new(RwLock::new(HashSet::new()));
        let input = "abd<a href=\"#1\"></a>abd".to_string();
        let expected = "abd<a href=\"#1\"></a>abd";
        let root_path = Path::new("/my/");
        let docdir = Path::new("/my/abs/note path/");
        let output = rewrite_links(
            input,
            root_path,
            docdir,
            LocalLinkKind::Short,
            false,
            allowed_urls.clone(),
        );
        let url = allowed_urls.read_recursive();
        println!("{:?}", allowed_urls.read_recursive());
        assert!(!url.contains(&PathBuf::from("/abs/note path/")));
        assert_eq!(output, expected);
    }

    #[test]
    fn test_rewrite_links_bare_fragment_all_modes() {
        use crate::config::LocalLinkKind;

        // The bug-report fixture: `[Chapter one](#ch1)` must resolve to
        // `#ch1` verbatim, regardless of `LocalLinkKind`.
        let root_path = Path::new("/my/");
        let docdir = Path::new("/my/abs/note path/");
        let input = "<a href=\"#ch1\">Chapter one</a>".to_string();
        let expected = "<a href=\"#ch1\">Chapter one</a>";

        for kind in [LocalLinkKind::Off, LocalLinkKind::Short, LocalLinkKind::Long] {
            let allowed_urls = Arc::new(RwLock::new(HashSet::new()));
            let output = rewrite_links(
                input.clone(),
                root_path,
                docdir,
                kind,
                false,
                allowed_urls,
            );
            assert_eq!(output, expected, "mode {kind:?} must leave a bare fragment untouched");
        }
    }

    /// The feature-request's own acceptance-test fixture, rendered as the
    /// HTML `pulldown-cmark` would already have produced (this tests
    /// `assign_heading_ids` in isolation, not the Markdown renderer).
    const HEADING_FIXTURE: &str = concat!(
        "<h2>Chapter one</h2>",
        "<h2 id=\"ch1\">Chapter one</h2>",
        "<h3>S9 — Check the PIN</h3>",
        "<h3>2. Second section</h3>",
        "<h3>Duplicate</h3>",
        "<h3>Duplicate</h3>",
        "<h3><em>Emphasis</em> and <code>code</code></h3>",
        "<h3>Ümlaut und Größe</h3>",
        "<h3>Trailing punctuation!</h3>",
    );

    #[test]
    fn test_assign_heading_ids_gfm() {
        use crate::config::HeadingIdPolicy;

        let expected = concat!(
            "<h2 id=\"chapter-one\">Chapter one</h2>",
            "<h2 id=\"ch1\">Chapter one</h2>",
            "<h3 id=\"s9--check-the-pin\">S9 — Check the PIN</h3>",
            "<h3 id=\"2-second-section\">2. Second section</h3>",
            "<h3 id=\"duplicate\">Duplicate</h3>",
            "<h3 id=\"duplicate-1\">Duplicate</h3>",
            "<h3 id=\"emphasis-and-code\"><em>Emphasis</em> and <code>code</code></h3>",
            "<h3 id=\"ümlaut-und-größe\">Ümlaut und Größe</h3>",
            "<h3 id=\"trailing-punctuation\">Trailing punctuation!</h3>",
        );

        let output = assign_heading_ids(HEADING_FIXTURE.to_string(), HeadingIdPolicy::Gfm);
        assert_eq!(output, expected);
        assert!(!output.contains("{#ch1}"), "raw heading-attribute syntax must never leak");
    }

    #[test]
    fn test_assign_heading_ids_pandoc() {
        use crate::config::HeadingIdPolicy;

        // Differs from Gfm on exactly one row: the leading `2. ` is
        // dropped entirely, rather than keeping the digit.
        let expected = concat!(
            "<h2 id=\"chapter-one\">Chapter one</h2>",
            "<h2 id=\"ch1\">Chapter one</h2>",
            "<h3 id=\"s9--check-the-pin\">S9 — Check the PIN</h3>",
            "<h3 id=\"second-section\">2. Second section</h3>",
            "<h3 id=\"duplicate\">Duplicate</h3>",
            "<h3 id=\"duplicate-1\">Duplicate</h3>",
            "<h3 id=\"emphasis-and-code\"><em>Emphasis</em> and <code>code</code></h3>",
            "<h3 id=\"ümlaut-und-größe\">Ümlaut und Größe</h3>",
            "<h3 id=\"trailing-punctuation\">Trailing punctuation!</h3>",
        );

        let output = assign_heading_ids(HEADING_FIXTURE.to_string(), HeadingIdPolicy::Pandoc);
        assert_eq!(output, expected);
    }

    #[test]
    fn test_assign_heading_ids_off() {
        use crate::config::HeadingIdPolicy;

        let output = assign_heading_ids(HEADING_FIXTURE.to_string(), HeadingIdPolicy::Off);
        assert_eq!(output, HEADING_FIXTURE, "Off must leave the HTML byte-for-byte unchanged");
    }

    #[test]
    fn test_assign_heading_ids_avoids_colliding_with_explicit_id() {
        use crate::config::HeadingIdPolicy;

        // A later auto-generated slug that would collide with an EARLIER
        // explicit `{#id}` must be disambiguated, not silently duplicated.
        let input = "<h2 id=\"duplicate\">Explicit</h2><h2>Duplicate</h2>".to_string();
        let expected = "<h2 id=\"duplicate\">Explicit</h2><h2 id=\"duplicate-1\">Duplicate</h2>";

        let output = assign_heading_ids(input, HeadingIdPolicy::Gfm);
        assert_eq!(output, expected);
    }

    /// A `#` in a directory name must not end up as a bare byte in the
    /// `href`: browsers read an unencoded `#` as the start of a fragment
    /// and never send anything after it, so the server only ever sees a
    /// truncated path. `rewrite_links` is the function shared by the
    /// viewer and `--export`, so this covers both call sites at once.
    #[test]
    fn test_rewrite_links_hash_in_dir_name() {
        use crate::config::LocalLinkKind;

        let allowed_urls = Arc::new(RwLock::new(HashSet::new()));
        let input = "<a href=\"01-Agenda.md\">link</a>".to_string();
        let root_path = Path::new("/notes/");
        let docdir = Path::new("/notes/Meeting #12-Project kickoff/");
        let output = rewrite_links(
            input,
            root_path,
            docdir,
            LocalLinkKind::Short,
            false,
            allowed_urls.clone(),
        );

        // The `#` that is part of the directory name is percent-encoded,
        // so the browser cannot mistake it for the start of a fragment.
        assert!(
            output.contains("href=\"/Meeting%20%2312-Project%20kickoff/01-Agenda.md\""),
            "unexpected output: {output}"
        );
        // No bare `#` remains in the href.
        assert!(!output.contains("Meeting #12"));

        // Bookkeeping still holds the raw, decoded filesystem path — this
        // is what the viewer compares an incoming (percent-decoded)
        // request path against, so encoding the `href` must not encode
        // this side too.
        let url = allowed_urls.read_recursive();
        assert!(url.contains(&PathBuf::from(
            "/Meeting #12-Project kickoff/01-Agenda.md"
        )));
    }

    #[test]
    fn test_is_empty_html() {
        // Bring new methods into scope.
        use crate::html::HtmlStr;

        // Test where input is '<!DOCTYPE html>'
        // See: [HTML doctype declaration](https://www.w3schools.com/tags/tag_doctype.ASP)
        assert!(String::from("<!DOCTYPE html>").is_empty_html());

        // This should fail:
        assert!(!String::from("<!DOCTYPE html>>").is_empty_html());

        // Test where input is '<!DOCTYPE html>'
        // See: [HTML doctype declaration](https://www.w3schools.com/tags/tag_doctype.ASP)
        assert!(
            String::from(
                " <!DOCTYPE HTML PUBLIC \
            \"-//W3C//DTD HTML 4.01 Transitional//EN\" \
            \"http://www.w3.org/TR/html4/loose.dtd\">"
            )
            .is_empty_html()
        );

        // Test where input is '<!DOCTYPE html>'
        // See: [HTML doctype declaration](https://www.w3schools.com/tags/tag_doctype.ASP)
        assert!(
            String::from(
                " <!DOCTYPE html PUBLIC \
            \"-//W3C//DTD XHTML 1.1//EN\" \
            \"http://www.w3.org/TR/xhtml11/DTD/xhtml11.dtd\">"
            )
            .is_empty_html()
        );

        // Test where input is '<!DOCTYPE html>Some content'
        assert!(!String::from("<!DOCTYPE html>Some content").is_empty_html());

        // Test where input is an empty string
        assert!(String::from("").is_empty_html());

        // Test where input is not empty HTML.
        // Convention: we consider empty only `` or `<!DOCTYPE html>`.
        assert!(!String::from("<html></html>").is_empty_html());

        // Test where input is not empty HTML with doctype
        // Convention: we consider empty only `` or `<!DOCTYPE html>`.
        assert!(!String::from("<!DOCTYPE html><html></html>").is_empty_html());
    }

    #[test]
    fn test_has_html_start_tag() {
        // Bring new methods into scope.
        use crate::html::HtmlStr;

        // Test where input is '<!DOCTYPE html>Some content'
        assert!(String::from("<!DOCTYPE html>Some content").has_html_start_tag());

        // This fails because we require be convention `<!DOCTYPE html>` as
        // first tag
        assert!(!String::from("<html>Some content</html>").has_html_start_tag());

        // This fails because we require be convention `<!DOCTYPE html>` as
        // first tag
        assert!(!String::from("<HTML>").has_html_start_tag());

        // Test where input starts with spaces
        assert!(String::from("  <!doctype html>Some content").has_html_start_tag());

        // Test where input is a non-HTML doctype
        assert!(!String::from("<!DOCTYPE other>").has_html_start_tag());

        // Test where input is an empty string
        assert!(!String::from("").has_html_start_tag());
    }

    #[test]
    fn test_is_html_unchecked() {
        // Bring new methods into scope.
        use crate::html::HtmlStr;

        // Test with `<!DOCTYPE html>` tag
        let html = "<!doctype html>";
        assert!(html.is_html_unchecked());

        // Test with `<!DOCTYPE html>` tag
        let html = "<!doctype html abc>def";
        assert!(html.is_html_unchecked());

        // Test with `<!DOCTYPE html>` tag
        let html = "<!doctype html";
        assert!(!html.is_html_unchecked());

        // Test with `<html>` tag
        let html = "<html><body></body></html>";
        assert!(html.is_html_unchecked());

        // Test with `<html>` tag
        let html = "<html abc>def";
        assert!(html.is_html_unchecked());

        // Test with `<html>` tag
        let html = "<html abc def";
        assert!(!html.is_html_unchecked());

        // Test with leading whitespace
        let html = "   <!doctype html><html><body></body></html>";
        assert!(html.is_html_unchecked());

        // Test with non-html content
        let html = "<!DOCTYPE xml><root></root>";
        assert!(!html.is_html_unchecked());

        // Test with partial `<!DOCTYPE>` tag
        let html = "<!doctype>";
        assert!(!html.is_html_unchecked());
    }

    #[test]
    fn test_prepend_html_start_tag() {
        // Bring new methods into scope.
        use crate::html::HtmlString;

        // Test where input already has doctype HTML
        assert_eq!(
            String::from("<!DOCTYPE html>Some content").prepend_html_start_tag(),
            Ok(String::from("<!DOCTYPE html>Some content"))
        );

        // Test where input already has doctype HTML
        assert_eq!(
            String::from("<!DOCTYPE html>").prepend_html_start_tag(),
            Ok(String::from("<!DOCTYPE html>"))
        );

        // Test where input has no HTML tag
        assert_eq!(
            String::from("<html>Some content").prepend_html_start_tag(),
            Ok(String::from("<!DOCTYPE html><html>Some content"))
        );

        // Test where input has a non-HTML doctype
        assert_eq!(
            String::from("<!DOCTYPE other>").prepend_html_start_tag(),
            Err(InputStreamError::NonHtmlDoctype {
                html: "<!DOCTYPE other>".to_string()
            })
        );

        // Test where input has no HTML tag
        assert_eq!(
            String::from("Some content").prepend_html_start_tag(),
            Ok(String::from("<!DOCTYPE html>Some content"))
        );

        // Test where input is an empty string
        assert_eq!(
            String::from("").prepend_html_start_tag(),
            Ok(String::from("<!DOCTYPE html>"))
        );
    }
}
