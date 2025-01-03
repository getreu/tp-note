//! Helper functions dealing with HTML conversion.
use crate::clone_ext::CloneExt;
use crate::error::InputStreamError;
use crate::filename::{NotePath, NotePathStr};
use crate::{config::LocalLinkKind, error::NoteError};
use html_escape;
use parking_lot::RwLock;
use parse_hyperlinks::parser::Link;
use parse_hyperlinks_extras::iterator_html::HtmlLinkInlineImage;
use percent_encoding::percent_decode_str;
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

/// If followed directly after FORMAT_SEPARATOR, it selects the the whole
/// filename for further matching.
const FORMAT_COMPLETE_FILENAME: &str = "?";

/// A format string can be separated in a _from_ and _to_ part. This
/// optional separator is placed after `FORMAT_SEPARATOR` and separates
/// the _from_ and _to_ pattern.
const FORMAT_FROM_TO_SEPARATOR: char = ':';

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

    // Caculate the output.
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
    /// Call this method only, when you sure that this
    /// is an autolink by testing with `is_autolink()`.
    fn rewrite_autolink(&mut self);

    /// A formatting attribute is a format string starting with `?  followed
    /// by one or two patterns. It is appended to to `dest` or `src`.
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
    /// we know, that the result will be inserted in an UTF-8 template.
    fn to_html(&self) -> String;
}

impl<'a> Hyperlink for Link<'a> {
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
                let dest_out = assemble_link(
                    root_path,
                    docdir,
                    Path::new(path.as_ref()),
                    rewrite_rel_paths,
                    rewrite_abs_paths,
                )
                .ok_or(NoteError::InvalidLocalPath {
                    path: path.as_ref().to_string(),
                })?;

                // Store result.
                let new_dest = Cow::Owned(dest_out.to_str().unwrap_or_default().to_string());
                let _ = std::mem::replace(path, new_dest);
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
                if !format.is_empty() {
                    if let Some(idx) = short_text.find(format) {
                        short_text = &short_text[..idx];
                    };
                }
            }
            // Some `:`
            Some((from, to)) => {
                if !from.is_empty() {
                    if let Some(idx) = short_text.find(from) {
                        short_text = &short_text[(idx + from.len())..];
                    };
                }
                if !to.is_empty() {
                    if let Some(idx) = short_text.find(to) {
                        short_text = &short_text[..idx];
                    };
                }
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
            // Strip URL fragment.
            match (dest.rfind('#'), dest.rfind(['/', '\\'])) {
                (Some(n), sep) if sep.is_some_and(|sep| n > sep) || sep.is_none() => {
                    Some(Path::new(&dest.as_ref()[..n]))
                }
                _ => Some(Path::new(dest.as_ref())),
            }
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
            let path = dest.as_ref();
            if path.has_tpnote_ext() {
                let mut newpath = path.to_string();
                newpath.push_str(HTML_EXT);

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
                // No cloning happens here, because we own s already.
                Cow::Owned(s.into_owned())
            }
        }
        // Replace Windows backslash, then HTML escape encode.
        fn repl_backspace_enc_amp(val: Cow<str>) -> Cow<str> {
            let val = if val.as_ref().contains('\\') {
                Cow::Owned(val.to_string().replace('\\', "/"))
            } else {
                val
            };
            let s = html_escape::encode_double_quoted_attribute(val.as_ref());
            if s == val {
                val
            } else {
                // No cloning happens here, because we own s already.
                Cow::Owned(s.into_owned())
            }
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
/// concerning absolute local links in Tp-Note documents: When a
/// document contains a local link with an absolute path (absolute local link),
/// the base of this path is considered to be the directory where the marker
/// file ‘.tpnote.toml’ resides (or ‘/’ in non exists). The marker file
/// directory is `root_path`.
/// Furthermore, the parameter `docdir` contains the absolute path of the
/// directory of the currently processed HTML document. The user guarantees that
/// `docdir` is the base for all relative local links in the document.
/// BTW: `docdir` must always start with `root_path`.
///
/// If `LocalLinkKind::Off`, relative local links are not converted.
/// If `LocalLinkKind::Short`, relative local links are converted into an
/// absolute local links with  `root_path` as base directory.
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

/// This trait deals with tagged HTML data.
pub struct HtmlStream;

impl HtmlStream {
    /// Pattern 1 to check if this is HTML.
    const START_TAG_PAT1: &'static str = "<html";
    /// Pattern 2 to check if this is HTML.
    const START_TAG_PAT2: &'static str = "<!doctype html";
    /// Lowercase test pattern to check if there is a document type declaration
    /// already.
    const START_TAG_PAT3: &'static str = "<!doctype ";
    /// Tag to insert if there is no start tag.
    const START_TAG: &'static str = "<!DOCTYPE html>";
    /// Return `true` when the input stream starts with  `<!DOCTYPE html`
    /// or `<html` (or lowercase variants).
    pub fn has_start_tag(html: &str) -> bool {
        let html2 = html
            .trim_start()
            .lines()
            .next()
            .map(|l| l.to_ascii_lowercase());
        html2.as_ref().is_some_and(|l| {
            l.starts_with(Self::START_TAG_PAT1) || l.starts_with(Self::START_TAG_PAT2)
        })
    }

    /// If the input does not start with `<!DOCTYPE html` or `<html`
    /// (or lowercase variants), then insert `<!DOCTYPE html>`.
    /// Returns `InputStreamError::NonHtmlDoctype` if there is another Doctype
    /// already.
    pub fn prepend_start_tag(html: String) -> Result<String, InputStreamError> {
        let html2 = html
            .trim_start()
            .lines()
            .next()
            .map(|l| l.to_ascii_lowercase());

        if html2.as_ref().is_some_and(|l| {
            l.starts_with(Self::START_TAG_PAT1) || l.starts_with(Self::START_TAG_PAT2)
        }) {
            // There is an HTML tag already. Nothing to do.
            return Ok(html);
        }

        if html2.is_some_and(|l| l.starts_with(Self::START_TAG_PAT3)) {
            // There is a Doctype other then HTML.
            return Err(InputStreamError::NonHtmlDoctype);
        }

        let mut html = html;
        html.insert_str(0, Self::START_TAG);
        Ok(html)
    }
}

#[cfg(test)]
mod tests {

    use crate::error::NoteError;
    use crate::html::assemble_link;
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

    use super::*;

    #[test]
    fn test_has_start_tag() {
        // Test cases where the start tag is present.
        assert!(HtmlStream::has_start_tag("<html>"));
        assert!(HtmlStream::has_start_tag("<!DOCTYPE html>"));
        assert!(HtmlStream::has_start_tag("   <html>"));
        assert!(HtmlStream::has_start_tag("   <!DOCTYPE html>"));

        // Test cases where the start tag is not present.
        assert!(!HtmlStream::has_start_tag("<!DOCTYPE other>"));
        assert!(!HtmlStream::has_start_tag("<body>"));
        assert!(!HtmlStream::has_start_tag("   <body>"));
    }

    #[test]
    fn test_prepend_start_tag() {
        // Test case where the start tag is already present.
        assert_eq!(
            HtmlStream::prepend_start_tag("<html>".to_string()).unwrap(),
            "<html>".to_string()
        );
        assert_eq!(
            HtmlStream::prepend_start_tag("<!DOCTYPE html>".to_string()).unwrap(),
            "<!DOCTYPE html>".to_string()
        );

        // Test case where the start tag needs to be inserted.
        assert_eq!(
            HtmlStream::prepend_start_tag("<body>".to_string()).unwrap(),
            "<!DOCTYPE html><body>".to_string()
        );

        // Test case where there is another doctype.
        assert_eq!(
            HtmlStream::prepend_start_tag("<!DOCTYPE other>".to_string()).unwrap_err(),
            InputStreamError::NonHtmlDoctype
        );
    }

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
        let expected = "<img src=\"/abs/note path/t m p.jpg\" \
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
        let expected = "<img src=\"/abs/t m p.jpg\" alt=\"Image\">";
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
        let expected = "<a href=\"/abs/note path/my note 1.md\">my note 1</a>";
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
        let expected = "<a href=\"/dir/my note 1.md\">my note 1</a>";
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
        let expected = "<a href=\"dir/my note 1.md\">my note 1</a>";
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
        let expected = "<a href=\"/path/dir/my note 1.md\">my note 1</a>";
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
        let expected = "<a href=\"/dir/my note 1.md\">my note 1</a>";
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
        let expected = "<a href=\"/path/dir/3.0-my note.md\">dir/3.0-my note.md</a>";
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
        let expected = "<a href=\"de&amp;&gt; st\" title=\"ti&amp;&gt; tle\">te&> xt</a>";
        let output = input.to_html();
        assert_eq!(output, expected);

        //
        let input = Link::Image(Cow::from("al&t"), Cow::from("sr&c"));
        let expected = "<img src=\"sr&amp;c\" alt=\"al&amp;t\">";
        let output = input.to_html();
        assert_eq!(output, expected);

        //
        let input = Link::Text2Dest(Cow::from("te&> xt"), Cow::from("de&> st"), Cow::from(""));
        let expected = "<a href=\"de&amp;&gt; st\">te&> xt</a>";
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
            ghi<img src=\"/abs/note path/t m p.jpg\" alt=\"test 1\">\
            jkl<a href=\"/abs/note path/down/my note 1.md\">my note 1</a>\
            mno<a href=\"/abs/note path/dir/my note.md\">./down/../dir/my note.md</a>\
            pqr<a href=\"/dir/my note.md\">/down/../dir/my note.md</a>\
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
        let expected = "abd<a href=\"/abs/note path/dir/my note.md\">\
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

        let allowed_urls = Arc::new(RwLock::new(HashSet::new()));
        let input = "abd<a href=\"#1\"></a>abd".to_string();
        let expected = "abd<a href=\"/abs/note path/#1\"></a>abd";
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
        assert!(url.contains(&PathBuf::from("/abs/note path/")));
        assert_eq!(output, expected);
    }
}
