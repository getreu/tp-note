//! Helper functions dealing with HTML conversion.
use crate::filename::{Extension, NotePathStr};
use crate::{config::LocalLinkKind, error::NoteError};
use html_escape;
use parking_lot::RwLock;
use parse_hyperlinks::parser::Link;
use parse_hyperlinks_extras::iterator_html::HyperlinkInlineImage;
use percent_encoding::percent_decode_str;
use std::{
    borrow::Cow,
    collections::HashSet,
    path::{Component, Path, PathBuf},
    sync::Arc,
};

pub(crate) const HTML_EXT: &str = ".html";

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
    fn decode_html_escape_and_percent(&mut self);

    /// True if the link is:
    /// * `Link::Text2Dest` and the link text equals the link destination, or
    /// * `Link::Image` and the links `alt` equals the the link source.
    /// Precondition: `decode_html_escape_and_percent()` must have been
    /// Executed.
    fn is_autolink(&self) -> bool;

    /// Member function converting the relative local URLs in `self`.
    /// If successful, we return `Ok(Some(URL))`, otherwise
    /// `Err(NoteError::InvalidLocalLink)`.
    /// If `self` contains an absolute URL, no conversion is performed and the
    /// return value is `Ok(None))`.
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
    /// Guaranties:
    /// 1. The returned link is guaranteed to be a child of `root_path`, or
    ///    `None`.
    fn rebase_local_link(
        &mut self,
        root_path: &Path,
        docdir: &Path,
        rewrite_rel_paths: bool,
        rewrite_abs_paths: bool,
        rewrite_ext: bool,
    ) -> Result<Option<PathBuf>, NoteError>;

    /// Extracts some substring in `dest`  (`Link::Text2Dest`) or
    /// `source` (`Link::Image`), and copy the result in `text`
    /// (`Link::Text2Dest`) or `alt` (`Link::Image`).
    /// For absolute URLs the scheme is removed and then copied.
    /// For paths in relative URLs only the file stem (without sort-tag,
    /// copy-counter and extension) is copied.
    /// WARNING: only execute this method if you have asserted before
    /// with `is_autolink()` that this is really an autolink.
    fn rewrite_autolink(&mut self);

    /// Renders `Link::Text2Dest` and `Link::Image` to HTML.
    /// Some characters are HTML escape encoded. URLs are not
    /// percent encoded, as the result will be inserted in an
    /// UTF-8 template. So percent encoding is not necessary.
    fn to_html(&self) -> String;
}

impl<'a> Hyperlink for Link<'a> {
    #[inline]
    fn decode_html_escape_and_percent(&mut self) {
        let empty_title = &mut Cow::from("");
        let (text, dest, title) = match self {
            Link::Text2Dest(text, dest, title) => (text, dest, title),
            Link::Image(alt, source) => (alt, source, empty_title),
            _ => unimplemented!(),
        };
        // HTML escape decoding
        {
            let decoded_text = html_escape::decode_html_entities(&*text);
            if matches!(&decoded_text, Cow::Owned(..)) {
                // Does nothing, but satisfying the borrow checker. Does not `clone()`.
                let decoded_text = Cow::Owned(decoded_text.into_owned());
                // Store result.
                let _ = std::mem::replace(text, decoded_text);
            }

            let decoded_dest = html_escape::decode_html_entities(&*dest);
            if matches!(&decoded_dest, Cow::Owned(..)) {
                // Does nothing, but satisfying the borrow checker. Does not `clone()`.
                let decoded_dest = Cow::Owned(decoded_dest.into_owned());
                // Store result.
                let _ = std::mem::replace(dest, decoded_dest);
            }

            let decoded_title = html_escape::decode_html_entities(&*title);
            if matches!(&decoded_title, Cow::Owned(..)) {
                // Does nothing, but satisfying the borrow checker. Does not `clone()`.
                let decoded_title = Cow::Owned(decoded_title.into_owned());
                // Store result.
                let _ = std::mem::replace(title, decoded_title);
            }
        }

        // Percent decode URL. The template we insert in is UTF-8 encoded.
        let decoded_dest = percent_decode_str(&*dest).decode_utf8().unwrap();
        if matches!(&decoded_dest, Cow::Owned(..)) {
            // Does nothing, but satisfying the borrow checker. Does not `clone()`.
            let decoded_dest = Cow::Owned(decoded_dest.into_owned());
            // Store result.
            let _ = std::mem::replace(dest, decoded_dest);
        }

        // The link text might be percent encoded in case of an autolink.
        let decoded_text = percent_decode_str(&*text).decode_utf8().unwrap();
        // Is this an autolink?
        if &decoded_text == dest {
            // Clone `dest` and store result.
            let _ = std::mem::replace(text, dest.clone());
        }
    }

    fn is_autolink(&self) -> bool {
        let (text, dest) = match self {
            Link::Text2Dest(text, dest, _title) => (text, dest),
            Link::Image(alt, source) => (alt, source),
            _ => return false,
        };
        text == dest
    }

    fn rebase_local_link(
        &mut self,
        root_path: &Path,
        docdir: &Path,
        rewrite_rel_paths: bool,
        rewrite_abs_paths: bool,
        rewrite_ext: bool,
    ) -> Result<Option<PathBuf>, NoteError> {
        //
        let (text, dest) = match self {
            Link::Text2Dest(text, dest, _title) => (text, dest),
            Link::Image(alt, source) => (alt, source),
            _ => return Err(NoteError::InvalidLocalLink),
        };

        // Return None, if link is not local.
        if (dest.contains("://") && !dest.contains(":///"))
            || dest.starts_with("mailto:")
            || dest.starts_with("tel:")
        {
            return Ok(None);
        }

        // Is this an autolink? Then modify `text`.
        if *text == *dest && (text.contains(':') || text.contains('@')) {
            let short_text = text
                .trim_start_matches("http://")
                .trim_start_matches("http:")
                .trim_start_matches("tpnote:");

            // Show only the stem as link text.
            // Strip the path.
            let short_text = short_text
                .rsplit_once(['/', '\\'])
                .map(|(_path, stem)| stem)
                .unwrap_or(short_text);

            // Strip extension...
            // The input `short_text` can be a full filename (starting with a
            // sort-tag, ending with an extension) or only a sort-tag.
            // In the latter case we do not strip anything.
            let sort_tag1 = short_text.split_sort_tag().0;
            let (sort_tag_stem, ext) = short_text.rsplit_once('.').unwrap_or((short_text, ""));
            let sort_tag2 = sort_tag_stem.split_sort_tag().0;
            // ... but only if the sort tag would not change and the extension
            // is a Tp-Note file.
            let short_text = if sort_tag1 == sort_tag2 && ext.is_tpnote_ext() {
                sort_tag_stem
            } else {
                short_text
            };
            // Store the result.
            let new_text = Cow::Owned(short_text.to_string());
            let _ = std::mem::replace(text, new_text);
            // Store the result.
        }

        // Now we deal with `dest`.
        {
            // As we have only local destinations here, we trim the URL scheme.
            let short_dest = dest
                .trim_start_matches("http://")
                .trim_start_matches("http:")
                .trim_start_matches("tpnote:");
            let short_dest = if let Cow::Owned(_) = dest {
                Cow::Owned(short_dest.to_string())
            } else {
                Cow::Borrowed(short_dest)
            };

            // Append ".html" to dest, if `rewrite_ext`.
            // Only rewrite file extensions for Tp-Note files.
            let short_dest = if rewrite_ext && dest.has_tpnote_ext() {
                Cow::Owned(format!("{}{}", short_dest, HTML_EXT))
            } else {
                short_dest
            };

            let dest_out = assemble_link(
                root_path,
                docdir,
                Path::new(&short_dest.as_ref()),
                rewrite_rel_paths,
                rewrite_abs_paths,
            )
            .ok_or(NoteError::InvalidLocalLink)?;

            // Store result.
            let new_dest = Cow::Owned(dest_out.to_str().unwrap_or_default().to_string());
            let _ = std::mem::replace(dest, new_dest);

            // Return `new_dest` as path.
            Ok(Some(dest_out))
        }
    }

    fn rewrite_autolink(&mut self) {
        let (text, dest) = match self {
            Link::Text2Dest(text, dest, _title) => (text, dest),
            Link::Image(alt, source) => (alt, source),
            _ => return,
        };

        // Is this an absolute URL?
        let is_rel_url = !((dest.contains("://") && !dest.contains(":///"))
            || dest.starts_with("mailto:")
            || dest.starts_with("tel:"));

        // Strip scheme from dest and copy the result in text.
        let short_dest = dest
            .trim_start_matches("https://")
            .trim_start_matches("https:")
            .trim_start_matches("http://")
            .trim_start_matches("http:")
            .trim_start_matches("tpnote:")
            .trim_start_matches("mailto:")
            .trim_start_matches("tel:")
            .to_string();
        if short_dest != dest.as_ref() {
            let _ = std::mem::replace(text, Cow::Owned(short_dest));
        }

        if is_rel_url {
            // Show only the stem as link text.
            // Strip the path.
            let short_text = text
                .as_ref()
                .rsplit_once(['/', '\\'])
                .map(|(_path, stem)| stem)
                .unwrap_or(text.as_ref());

            // Strip extension...
            // The input `short_text` can be a full filename (starting with a
            // sort-tag, ending with an extension) or only a sort-tag.
            // In the latter case we do not strip anything.
            let sort_tag1 = short_text.split_sort_tag().0;
            let (sort_tag_stem, ext) = short_text.rsplit_once('.').unwrap_or((short_text, ""));
            let sort_tag2 = sort_tag_stem.split_sort_tag().0;
            // ... but only if the sort tag would not change and the extension
            // is a Tp-Note file.
            let short_text = if sort_tag1 == sort_tag2 && ext.is_tpnote_ext() {
                sort_tag_stem
            } else {
                short_text
            };
            // Store the result.
            if short_text != text.as_ref() {
                let _ = std::mem::replace(text, Cow::Owned(short_text.to_string()));
            }
        }
    }

    //
    fn to_html(&self) -> String {
        let empty_title = &Cow::from("");
        let (text, dest, title) = match self {
            Link::Text2Dest(text, dest, title) => (text, dest, title),
            Link::Image(alt, source) => (alt, source, empty_title),
            _ => unimplemented!(),
        };
        // Replace Windows backslash
        let dest = if (*dest).contains('\\') {
            Cow::Owned(dest.to_string().replace('\\', "/"))
        } else {
            Cow::Borrowed(&**dest)
        };

        // Encode HTML escape.
        let text = html_escape::encode_text(&text);
        let dest = html_escape::encode_double_quoted_attribute(&dest);
        let title = html_escape::encode_double_quoted_attribute(&title);

        // Format title.
        let title = if !title.is_empty() {
            format!(" title=\"{}\"", title)
        } else {
            title.to_string()
        };
        // Save results.

        match self {
            Link::Text2Dest(_, _, _) => format!("<a href=\"{}\"{}>{}</a>", dest, title, text),
            Link::Image(_, _) => format!("<img src=\"{}\" alt=\"{}\" />", dest, text),
            _ => unimplemented!(),
        }
    }
}

#[inline]
/// A helper function that scans the input HTML document in `html_input` for
/// HTML hyperlinks. When it finds a relative URL (local link), it analyzes it's
/// path.  A relative path is then converted into an absolute path, before the
/// result is reinserted into the HTML document.
///
/// The base path for this conversion is `docdir`, the location of the HTML
/// document.
/// If not `rewrite_rel_paths`, relative local paths are not converted.
/// Furthermore, all local _absolute_ (not converted) paths are prepended with
/// `root_path`. All external URLs always remain untouched.
/// If `rewrite_abs_paths` and the URL's path is absolute, it prepends
/// `root_path`.
/// Finally, if `rewrite_ext` is true and a local link points to a known
/// Tp-Note file extension, then `.html` is appended to the converted link.
/// Remark: The link's text property is never changed. However, there is
/// one exception: when the link's text contains a string similar to URLs,
/// starting with `http:` or `tpnote:`. In this case, the string is interpreted
/// URL and only the stem of the filename is displayed, e.g.
/// `<a ...>http:dir/my file.md</a>` is rewritten into `<a ...>my file</a>`.
///
/// Before a local (converted) link is reinserted in the output HTML,
/// a copy is inserted into `allowed_local_links` for further bookkeeping.
///
/// decoded. After rewriting, the links are finally HTML escape encoded before
/// the are reinserted in the output HTML of this function.
/// NB2: It is guaranteed, that the resulting HTML document contains only local links
/// to other documents within `root_path`. Deviant links displayed as `INVALID
/// LOCAL LINK` and URL is discarded.
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

    let mut allowed_urls = allowed_local_links.write();

    // Search for hyperlinks and inline images in the HTML rendition
    // of this note.
    let mut rest = &*html_input;
    let mut html_out = String::new();
    for ((skipped, _consumed, remaining), mut link) in HyperlinkInlineImage::new(&html_input) {
        html_out.push_str(skipped);
        rest = remaining;

        // Percent decode link destination.
        link.decode_html_escape_and_percent();
        // Rewrite the local link.
        match link.rebase_local_link(
            root_path,
            docdir,
            rewrite_rel_paths,
            rewrite_abs_paths,
            rewrite_ext,
        ) {
            Ok(Some(dest_path)) => {
                allowed_urls.insert(dest_path);
                html_out.push_str(&link.to_html());
            }
            Ok(None) => html_out.push_str(&link.to_html()),

            Err(e) => html_out.push_str(&e.to_string()),
        };
    }
    // Add the last `remaining`.
    html_out.push_str(rest);

    if allowed_urls.is_empty() {
        log::debug!(
            "Viewer: note file has no local hyperlinks. No additional local files are served.",
        );
    } else {
        log::debug!(
            "Viewer: referenced allowed local files: {}",
            allowed_urls
                .iter()
                .map(|p| {
                    let mut s = "\n    '".to_string();
                    s.push_str(&p.display().to_string());
                    s
                })
                .collect::<String>()
        );
    }

    html_out
    // The `RwLockWriteGuard` is released here.
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

    use super::Hyperlink;

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
        input.decode_html_escape_and_percent();
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
        input.decode_html_escape_and_percent();
        let output = input;
        assert_eq!(output, expected);

        //
        let mut input = Link::Text2Dest(
            Cow::from("d:e%20st"),
            Cow::from("d:e%20st"),
            Cow::from("title"),
        );
        let expected =
            Link::Text2Dest(Cow::from("d:e st"), Cow::from("d:e st"), Cow::from("title"));
        input.decode_html_escape_and_percent();
        let output = input;
        assert_eq!(output, expected);

        let mut input = Link::Text2Dest(
            Cow::from("d:e%20&st%26"),
            Cow::from("d:e%20%26st&"),
            Cow::from("title"),
        );
        let expected = Link::Text2Dest(
            Cow::from("d:e &st&"),
            Cow::from("d:e &st&"),
            Cow::from("title"),
        );
        input.decode_html_escape_and_percent();
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
        input.decode_html_escape_and_percent();
        let output = input;
        assert_eq!(output, expected);

        //
        let mut input = Link::Image(Cow::from("al%20t"), Cow::from("de%20st"));
        let expected = Link::Image(Cow::from("al%20t"), Cow::from("de st"));
        input.decode_html_escape_and_percent();
        let output = input;
        assert_eq!(output, expected);

        //
        let mut input = Link::Image(Cow::from("a\\lt"), Cow::from("d\\est"));
        let expected = Link::Image(Cow::from("a\\lt"), Cow::from("d\\est"));
        input.decode_html_escape_and_percent();
        let output = input;
        assert_eq!(output, expected);

        //
        let mut input = Link::Image(Cow::from("a&amp;&quot;lt"), Cow::from("a&amp;&quot;lt"));
        let expected = Link::Image(Cow::from("a&\"lt"), Cow::from("a&\"lt"));
        input.decode_html_escape_and_percent();
        let output = input;
        assert_eq!(output, expected);
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
        assert!(input
            .rebase_local_link(root_path, docdir, true, false, false)
            .unwrap()
            .is_none());

        //
        let root_path = Path::new("/my/");
        let docdir = Path::new("/my/abs/note path/");

        // Check relative path to image.
        let mut input = take_link("<img src=\"down/./down/../../t m p.jpg\" alt=\"Image\" />")
            .unwrap()
            .1
             .1;
        let expected = "<img src=\"/abs/note path/t m p.jpg\" \
            alt=\"Image\" />";
        let outpath = input
            .rebase_local_link(root_path, docdir, true, false, false)
            .unwrap()
            .unwrap();
        let output = input.to_html();
        assert_eq!(output, expected);
        assert_eq!(outpath, PathBuf::from("/abs/note path/t m p.jpg"));

        // Check relative path to image. Canonicalized?
        let mut input = take_link("<img src=\"down/./../../t m p.jpg\" alt=\"Image\" />")
            .unwrap()
            .1
             .1;
        let expected = "<img src=\"/abs/t m p.jpg\" alt=\"Image\" />";
        let outpath = input
            .rebase_local_link(root_path, docdir, true, false, false)
            .unwrap()
            .unwrap();
        let output = input.to_html();
        assert_eq!(output, expected);
        assert_eq!(outpath, PathBuf::from("/abs/t m p.jpg"));

        // Check relative path to note file.
        let mut input = take_link("<a href=\"./down/./../my note 1.md\">my note 1</a>")
            .unwrap()
            .1
             .1;
        let expected = "<a href=\"/abs/note path/my note 1.md\">my note 1</a>";
        let outpath = input
            .rebase_local_link(root_path, docdir, true, false, false)
            .unwrap()
            .unwrap();
        let output = input.to_html();
        assert_eq!(output, expected);
        assert_eq!(outpath, PathBuf::from("/abs/note path/my note 1.md"));

        // Check absolute path to note file.
        let mut input = take_link("<a href=\"/dir/./down/../my note 1.md\">my note 1</a>")
            .unwrap()
            .1
             .1;
        let expected = "<a href=\"/dir/my note 1.md\">my note 1</a>";
        let outpath = input
            .rebase_local_link(root_path, docdir, true, false, false)
            .unwrap()
            .unwrap();
        let output = input.to_html();
        assert_eq!(output, expected);
        assert_eq!(outpath, PathBuf::from("/dir/my note 1.md"));

        // Check relative path to note file. Canonicalized?
        let mut input = take_link("<a href=\"./down/./../dir/my note 1.md\">my note 1</a>")
            .unwrap()
            .1
             .1;
        let expected = "<a href=\"dir/my note 1.md\">my note 1</a>";
        let outpath = input
            .rebase_local_link(root_path, docdir, false, false, false)
            .unwrap()
            .unwrap();
        let output = input.to_html();
        assert_eq!(output, expected);
        assert_eq!(outpath, PathBuf::from("dir/my note 1.md"));

        // Check `rewrite_ext=true`.
        let mut input = take_link("<a href=\"./down/./../dir/my note 1.md\">my note 1</a>")
            .unwrap()
            .1
             .1;
        let expected = "<a href=\"/abs/note path/dir/my note 1.md.html\">my note 1</a>";
        let outpath = input
            .rebase_local_link(root_path, docdir, true, false, true)
            .unwrap()
            .unwrap();
        let output = input.to_html();
        assert_eq!(output, expected);
        assert_eq!(
            outpath,
            PathBuf::from("/abs/note path/dir/my note 1.md.html")
        );

        // Check relative link in input.
        let mut input = take_link("<a href=\"./down/./../dir/my note 1.md\">my note 1</a>")
            .unwrap()
            .1
             .1;
        let expected = "<a href=\"/path/dir/my note 1.md\">my note 1</a>";
        let outpath = input
            .rebase_local_link(
                Path::new("/my/note/"),
                Path::new("/my/note/path/"),
                true,
                false,
                false,
            )
            .unwrap()
            .unwrap();
        let output = input.to_html();
        assert_eq!(output, expected);
        assert_eq!(outpath, PathBuf::from("/path/dir/my note 1.md"));

        // Check absolute link in input.
        let mut input = take_link("<a href=\"/down/./../dir/my note 1.md\">my note 1</a>")
            .unwrap()
            .1
             .1;
        let expected = "<a href=\"/dir/my note 1.md\">my note 1</a>";
        let outpath = input
            .rebase_local_link(root_path, Path::new("/my/ignored/"), true, false, false)
            .unwrap()
            .unwrap();
        let output = input.to_html();
        assert_eq!(output, expected);
        assert_eq!(outpath, PathBuf::from("/dir/my note 1.md"));

        // Check absolute link in input, not in `root_path`.
        let mut input = take_link("<a href=\"/down/../../dir/my note 1.md\">my note 1</a>")
            .unwrap()
            .1
             .1;
        let output = input
            .rebase_local_link(root_path, Path::new("/my/notepath/"), true, false, false)
            .unwrap_err();
        assert!(matches!(output, NoteError::InvalidLocalLink));

        // Check relative link in input, not in `root_path`.
        let mut input = take_link("<a href=\"../../dir/my note 1.md\">my note 1</a>")
            .unwrap()
            .1
             .1;
        let output = input
            .rebase_local_link(root_path, Path::new("/my/notepath/"), true, false, false)
            .unwrap_err();
        assert!(matches!(output, NoteError::InvalidLocalLink));

        // Check relative link in input, with underflow.
        let root_path = Path::new("/");
        let mut input = take_link("<a href=\"../../dir/my note 1.md\">my note 1</a>")
            .unwrap()
            .1
             .1;
        let output = input
            .rebase_local_link(root_path, Path::new("/my/"), true, false, false)
            .unwrap_err();
        assert!(matches!(output, NoteError::InvalidLocalLink));

        // Check relative link in input, not in `root_path`.
        let root_path = Path::new("/my");
        let mut input = take_link("<a href=\"../../dir/my note 1.md\">my note 1</a>")
            .unwrap()
            .1
             .1;
        let output = input
            .rebase_local_link(root_path, Path::new("/my/notepath"), true, false, false)
            .unwrap_err();
        assert!(matches!(output, NoteError::InvalidLocalLink));

        // Test autolink.
        let root_path = Path::new("/my");
        let mut input =
            take_link("<a href=\"tpnote:dir/3.0-my note.md\">tpnote:dir/3.0-my note.md</a>")
                .unwrap()
                .1
                 .1;
        let outpath = input
            .rebase_local_link(root_path, Path::new("/my/path"), true, false, false)
            .unwrap()
            .unwrap();
        let output = input.to_html();
        let expected = "<a href=\"/path/dir/3.0-my note.md\">3.0-my note</a>";
        assert_eq!(output, expected);
        assert_eq!(outpath, PathBuf::from("/path/dir/3.0-my note.md"));

        // Test short autolink 1 with sort-tag only.
        let root_path = Path::new("/my");
        let mut input = take_link("<a href=\"tpnote:dir/3.0\">tpnote:dir/3.0</a>")
            .unwrap()
            .1
             .1;
        let outpath = input
            .rebase_local_link(root_path, Path::new("/my/path"), true, false, false)
            .unwrap()
            .unwrap();
        let output = input.to_html();
        let expected = "<a href=\"/path/dir/3.0\">3.0</a>";
        assert_eq!(output, expected);
        assert_eq!(outpath, PathBuf::from("/path/dir/3.0"));
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
            Cow::from("/dir/3.0-My note.md"),
            Cow::from("/dir/3.0-My note.md"),
            Cow::from("title"),
        );
        let expected = Link::Text2Dest(
            Cow::from("3.0-My note"),
            Cow::from("/dir/3.0-My note.md"),
            Cow::from("title"),
        );
        input.rewrite_autolink();
        let output = input;
        assert_eq!(output, expected);

        //
        let mut input = Link::Text2Dest(
            Cow::from("/dir/3.0"),
            Cow::from("/dir/3.0"),
            Cow::from("title"),
        );
        let expected = Link::Text2Dest(Cow::from("3.0"), Cow::from("/dir/3.0"), Cow::from("title"));
        input.rewrite_autolink();
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
        let expected = "<a href=\"de&amp;&gt; st\" title=\"ti&amp;&gt; tle\">te&amp;&gt; xt</a>";
        let output = input.to_html();
        assert_eq!(output, expected);

        //
        let input = Link::Image(Cow::from("al&> t"), Cow::from("so&> urce"));
        let expected = "<img src=\"so&amp;&gt; urce\" alt=\"al&amp;&gt; t\" />";
        let output = input.to_html();
        assert_eq!(output, expected);

        //
        let input = Link::Text2Dest(Cow::from("te&> xt"), Cow::from("de&> st"), Cow::from(""));
        let expected = "<a href=\"de&amp;&gt; st\">te&amp;&gt; xt</a>";
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
            mno<a href=\"http:./down/../dir/my note.md\">\
            http:./down/../dir/my note.md</a>\
            pqr<a href=\"http:/down/../dir/my note.md\">\
            http:/down/../dir/my note.md</a>\
            stu<a href=\"http:/../dir/underflow/my note.md\">\
            not allowed dir</a>\
            vwx<a href=\"http:../../../not allowed dir/my note.md\">\
            not allowed</a>"
            .to_string();
        let expected = "abc<a href=\"ftp://getreu.net\">Blog</a>\
            def<a href=\"https://getreu.net\">https://getreu.net</a>\
            ghi<img src=\"/abs/note path/t m p.jpg\" alt=\"test 1\" />\
            jkl<a href=\"/abs/note path/down/my note 1.md\">my note 1</a>\
            mno<a href=\"/abs/note path/dir/my note.md\">my note</a>\
            pqr<a href=\"/dir/my note.md\">my note</a>\
            stu<i>INVALID LOCAL LINK</i>\
            vwx<i>INVALID LOCAL LINK</i>"
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

        assert_eq!(output, expected);
        assert!(url.contains(&PathBuf::from("/abs/note path/t m p.jpg")));
        assert!(url.contains(&PathBuf::from("/abs/note path/dir/my note.md")));
        assert!(url.contains(&PathBuf::from("/abs/note path/down/my note 1.md")));
    }
}
