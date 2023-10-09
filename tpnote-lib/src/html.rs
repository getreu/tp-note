//! Helper functions dealing with HTML conversion.

use crate::markup_language::MarkupLanguage;
use crate::{config::LocalLinkKind, error::NoteError};
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

/// If `rewrite_rel_links` and `dest` is relative, concat `docdir`  and
/// `dest`, then strip `root_path` from the left before returning.
/// If not `rewrite_rel_links` and `dest` is relative, return `dest`.
/// If `rewrite_abs_links` and `dest` is absolute, concatenate and return
/// `root_path` and `dest`.
/// If not `rewrite_abs_links` and `dest` is absolute, return `dest`.
/// The `dest` portion of the output is always canonicalized.
/// Return the assembled path, when in `root_path`, or `None` otherwise.
/// Asserts in debug mode, that `doc_dir` is in `root_path`.
fn assemble_link(
    root_path: &Path,
    docdir: &Path,
    dest: &Path,
    rewrite_rel_links: bool,
    rewrite_abs_links: bool,
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
    let mut link = match (rewrite_rel_links, rewrite_abs_links, dest_is_relative) {
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
    /// A helper function, that percent decodes the link destinations (and the
    /// link text in case of an autolink).
    fn percent_decode_url(&mut self);

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
    ///
    /// Guaranties:
    /// 1. The returned link is guaranteed to be a child of `root_path`, or
    ///    `None`.
    fn rewrite_local_link(
        &mut self,
        root_path: &Path,
        docdir: &Path,
        rewrite_rel_links: bool,
        rewrite_abs_links: bool,
        rewrite_ext: bool,
    ) -> Result<Option<PathBuf>, NoteError>;

    /// Renders `Link::Text2Dest` and `Link::Image` to HTML.
    fn render_html(&self) -> String;
}

impl<'a> Hyperlink for Link<'a> {
    #[inline]
    fn percent_decode_url(&mut self) {
        match self {
            Link::Text2Dest(text, dest, _title) => {
                // Is this an autolink?
                let autolink = text == dest;

                // Percent decode URL. The template we insert in is UTF-8 encoded.
                let decoded_dest = percent_decode_str(&*dest).decode_utf8().unwrap();
                if matches!(&decoded_dest, Cow::Owned(..)) {
                    let decoded_dest = Cow::Owned(decoded_dest.to_string());
                    // Store result.
                    let _ = std::mem::replace(dest, decoded_dest);
                }

                // We check also if `decoded_dest==decoded_text`.
                let decoded_text = percent_decode_str(&*text).decode_utf8().unwrap();
                let autolink = match (&text, decoded_text) {
                    (Cow::Borrowed(_), Cow::Borrowed(_)) => autolink,
                    (_, decoded_text) => &decoded_text == dest,
                };
                if autolink {
                    // Store result.
                    let _ = std::mem::replace(text, dest.clone());
                }
            }

            Link::Image(_alt_text, source) => {
                // Percent decode URL. The template we insert in is UTF-8 encoded.
                let decoded_source = percent_decode_str(&*source).decode_utf8().unwrap();
                if matches!(&decoded_source, Cow::Owned(..)) {
                    let decoded_source = Cow::Owned(decoded_source.to_string());
                    // Store result.
                    let _ = std::mem::replace(source, decoded_source);
                }
            }

            // The `HyperlinkInlineImage` iterator has no further variants.
            _ => unreachable!(),
        }
    }

    fn rewrite_local_link(
        &mut self,
        root_path: &Path,
        docdir: &Path,
        rewrite_rel_links: bool,
        rewrite_abs_links: bool,
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
        if text == dest && (text.contains(':') || text.contains('@')) {
            let short_text = text
                .trim_start_matches("http://")
                .trim_start_matches("http:")
                .trim_start_matches("tpnote:");
            // Strip extension.
            let short_text = short_text
                .rsplit_once('.')
                .map(|(s, _ext)| s)
                .unwrap_or(short_text);
            // Show only the stem as link text. Strip path.
            let short_text = short_text
                .rsplit_once(['/', '\\'])
                .map(|(_path, stem)| stem)
                .unwrap_or(short_text);
            // Store result.
            let new_text = Cow::Owned(short_text.to_string());
            let _ = std::mem::replace(text, new_text);
            // Store result.
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
            let short_dest =
                if rewrite_ext && MarkupLanguage::from(Path::new(dest.as_ref())).is_some() {
                    Cow::Owned(format!("{}{}", short_dest, HTML_EXT))
                } else {
                    short_dest
                };

            let dest_out = assemble_link(
                root_path,
                docdir,
                Path::new(&short_dest.as_ref()),
                rewrite_rel_links,
                rewrite_abs_links,
            )
            .ok_or(NoteError::InvalidLocalLink)?;

            // Store result.
            let new_dest = Cow::Owned(dest_out.to_str().unwrap_or_default().to_string());
            let _ = std::mem::replace(dest, new_dest);

            // Return `new_dest` as path.
            Ok(Some(dest_out))
        }
    }

    //
    fn render_html(&self) -> String {
        match &self {
            Link::Text2Dest(text, dest, title) => {
                // Replace Windows backslash
                let newdest = if (*dest).contains('\\') {
                    Cow::Owned(dest.to_string().replace('\\', "/"))
                } else {
                    dest.clone()
                };
                let title = if !title.is_empty() {
                    format!(" title=\"{}\"", title)
                } else {
                    title.to_string()
                };
                // Save results.
                format!("<a href=\"{}\"{}>{}</a>", newdest, title, text)
            }

            Link::Image(text, dest) => {
                // Replace Windows backslash
                let newdest = if (*dest).contains('\\') {
                    Cow::Owned(dest.to_string().replace('\\', "/"))
                } else {
                    dest.clone()
                };

                format!("<img src=\"{}\" alt=\"{}\" />", newdest, text)
            }

            _ => String::new(),
        }
    }
}

#[inline]
/// Helper function that scans the input `html` string and converts
/// all relative local HTML links to absolute links.
///
/// The base path for this conversion (usually where the HTML file resides),
/// is `docdir`.
/// If not `rewrite_rel_links`, relative local links are not converted.
/// Furthermore, all local _absolute_ (not converted) links are prepended with
/// `root_path`. All external URLs always remain untouched.
/// If `rewrite_abs_links` and `link` is absolute, concatenate and return
/// `root_path` and `dest`.
/// If `rewrite_ext` is true and the link points to a known Tp-Note file
/// extension, then `.html` is appended to the converted link.
/// Remark: The _anchor's text property_ is never changed. However, there is
/// one exception: when the text contains a URL starting with `http:` or
/// `https:`, only the file stem is kept. Example, the anchor text property:
/// `<a ...>http:dir/my file.md</a>` is rewritten into `<a ...>my file</a>`.
///
/// It is guaranteed, that all local links in the converted `html` point inside
/// `root_path`. If not, the link is displayed as `INVALID LOCAL LINK` and
/// discarded. All valid local links are inserted in `allowed_local_links`
/// the same way as their destination appears in the resulting HTML.
/// NB: Before processing, all links are percent decoded, as some
/// rendering libraries do not do this, e.g. `pulldown-cmark`.
pub fn rewrite_links(
    html: String,
    root_path: &Path,
    docdir: &Path,
    local_link_kind: LocalLinkKind,
    rewrite_ext: bool,
    allowed_local_links: Arc<RwLock<HashSet<PathBuf>>>,
) -> String {
    let (rewrite_rel_links, rewrite_abs_links) = match local_link_kind {
        LocalLinkKind::Off => (false, false),
        LocalLinkKind::Short => (true, false),
        LocalLinkKind::Long => (true, true),
    };

    let mut allowed_urls = allowed_local_links.write();

    // Search for hyperlinks and inline images in the HTML rendition
    // of this note.
    let mut rest = &*html;
    let mut html_out = String::new();
    for ((skipped, _consumed, remaining), mut link) in HyperlinkInlineImage::new(&html) {
        html_out.push_str(skipped);
        rest = remaining;

        // Percent decode link destination.
        link.percent_decode_url();

        // Rewrite the local link.
        match link.rewrite_local_link(
            root_path,
            docdir,
            rewrite_rel_links,
            rewrite_abs_links,
            rewrite_ext,
        ) {
            Ok(Some(dest_path)) => {
                allowed_urls.insert(dest_path);
                html_out.push_str(&link.render_html());
            }
            Ok(None) => html_out.push_str(&link.render_html()),

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
    fn test_rewrite_link1() {
        let root_path = Path::new("/my/");
        let docdir = Path::new("/my/abs/note path/");

        // Should panic: this is not a relative path.
        let mut input = take_link("<a href=\"ftp://getreu.net\">Blog</a>")
            .unwrap()
            .1
             .1;
        assert!(input
            .rewrite_local_link(root_path, docdir, true, false, false)
            .unwrap()
            .is_none());
    }

    #[test]
    fn test_rewrite_link2() {
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
            .rewrite_local_link(root_path, docdir, true, false, false)
            .unwrap()
            .unwrap();
        let output = input.render_html();
        assert_eq!(output, expected);
        assert_eq!(outpath, PathBuf::from("/abs/note path/t m p.jpg"));

        // Check relative path to image. Canonicalized?
        let mut input = take_link("<img src=\"down/./../../t m p.jpg\" alt=\"Image\" />")
            .unwrap()
            .1
             .1;
        let expected = "<img src=\"/abs/t m p.jpg\" alt=\"Image\" />";
        let outpath = input
            .rewrite_local_link(root_path, docdir, true, false, false)
            .unwrap()
            .unwrap();
        let output = input.render_html();
        assert_eq!(output, expected);
        assert_eq!(outpath, PathBuf::from("/abs/t m p.jpg"));

        // Check relative path to note file.
        let mut input = take_link("<a href=\"./down/./../my note 1.md\">my note 1</a>")
            .unwrap()
            .1
             .1;
        let expected = "<a href=\"/abs/note path/my note 1.md\">my note 1</a>";
        let outpath = input
            .rewrite_local_link(root_path, docdir, true, false, false)
            .unwrap()
            .unwrap();
        let output = input.render_html();
        assert_eq!(output, expected);
        assert_eq!(outpath, PathBuf::from("/abs/note path/my note 1.md"));

        // Check absolute path to note file.
        let mut input = take_link("<a href=\"/dir/./down/../my note 1.md\">my note 1</a>")
            .unwrap()
            .1
             .1;
        let expected = "<a href=\"/dir/my note 1.md\">my note 1</a>";
        let outpath = input
            .rewrite_local_link(root_path, docdir, true, false, false)
            .unwrap()
            .unwrap();
        let output = input.render_html();
        assert_eq!(output, expected);
        assert_eq!(outpath, PathBuf::from("/dir/my note 1.md"));

        // Check relative path to note file. Canonicalized?
        let mut input = take_link("<a href=\"./down/./../dir/my note 1.md\">my note 1</a>")
            .unwrap()
            .1
             .1;
        let expected = "<a href=\"dir/my note 1.md\">my note 1</a>";
        let outpath = input
            .rewrite_local_link(root_path, docdir, false, false, false)
            .unwrap()
            .unwrap();
        let output = input.render_html();
        assert_eq!(output, expected);
        assert_eq!(outpath, PathBuf::from("dir/my note 1.md"));

        // Check `rewrite_ext=true`.
        let mut input = take_link("<a href=\"./down/./../dir/my note 1.md\">my note 1</a>")
            .unwrap()
            .1
             .1;
        let expected = "<a href=\"/abs/note path/dir/my note 1.md.html\">my note 1</a>";
        let outpath = input
            .rewrite_local_link(root_path, docdir, true, false, true)
            .unwrap()
            .unwrap();
        let output = input.render_html();
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
            .rewrite_local_link(
                Path::new("/my/note/"),
                Path::new("/my/note/path/"),
                true,
                false,
                false,
            )
            .unwrap()
            .unwrap();
        let output = input.render_html();
        assert_eq!(output, expected);
        assert_eq!(outpath, PathBuf::from("/path/dir/my note 1.md"));

        // Check absolute link in input.
        let mut input = take_link("<a href=\"/down/./../dir/my note 1.md\">my note 1</a>")
            .unwrap()
            .1
             .1;
        let expected = "<a href=\"/dir/my note 1.md\">my note 1</a>";
        let outpath = input
            .rewrite_local_link(root_path, Path::new("/my/ignored/"), true, false, false)
            .unwrap()
            .unwrap();
        let output = input.render_html();
        assert_eq!(output, expected);
        assert_eq!(outpath, PathBuf::from("/dir/my note 1.md"));

        // Check absolute link in input, not in `root_path`.
        let mut input = take_link("<a href=\"/down/../../dir/my note 1.md\">my note 1</a>")
            .unwrap()
            .1
             .1;
        let output = input
            .rewrite_local_link(root_path, Path::new("/my/notepath/"), true, false, false)
            .unwrap_err();
        assert!(matches!(output, NoteError::InvalidLocalLink));

        // Check relative link in input, not in `root_path`.
        let mut input = take_link("<a href=\"../../dir/my note 1.md\">my note 1</a>")
            .unwrap()
            .1
             .1;
        let output = input
            .rewrite_local_link(root_path, Path::new("/my/notepath/"), true, false, false)
            .unwrap_err();
        assert!(matches!(output, NoteError::InvalidLocalLink));

        // Check relative link in input, with underflow.
        let root_path = Path::new("/");
        let mut input = take_link("<a href=\"../../dir/my note 1.md\">my note 1</a>")
            .unwrap()
            .1
             .1;
        let output = input
            .rewrite_local_link(root_path, Path::new("/my/"), true, false, false)
            .unwrap_err();
        assert!(matches!(output, NoteError::InvalidLocalLink));

        // Check relative link in input, not in `root_path`.
        let root_path = Path::new("/my");
        let mut input = take_link("<a href=\"../../dir/my note 1.md\">my note 1</a>")
            .unwrap()
            .1
             .1;
        let output = input
            .rewrite_local_link(root_path, Path::new("/my/notepath"), true, false, false)
            .unwrap_err();
        assert!(matches!(output, NoteError::InvalidLocalLink));
    }

    #[test]
    fn test_percent_decode() {
        //
        let mut input = Link::Text2Dest(Cow::from("text"), Cow::from("dest"), Cow::from("title"));
        let expected = Link::Text2Dest(Cow::from("text"), Cow::from("dest"), Cow::from("title"));
        input.percent_decode_url();
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
        input.percent_decode_url();
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
        input.percent_decode_url();
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
        input.percent_decode_url();
        let output = input;
        assert_eq!(output, expected);

        //
        let mut input = Link::Image(Cow::from("al%20t"), Cow::from("de%20st"));
        let expected = Link::Image(Cow::from("al%20t"), Cow::from("de st"));
        input.percent_decode_url();
        let output = input;
        assert_eq!(output, expected);

        //
        let mut input = Link::Image(Cow::from("a\\lt"), Cow::from("d\\est"));
        let expected = Link::Image(Cow::from("a\\lt"), Cow::from("d\\est"));
        input.percent_decode_url();
        let output = input;
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
