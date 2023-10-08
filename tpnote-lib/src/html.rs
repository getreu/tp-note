//! Helper functions dealing with HTML conversion.

use crate::config::LocalLinkKind;
use crate::markup_language::MarkupLanguage;
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

#[inline]
/// Helper function that converts relative local HTML links to absolute
/// links. If successful, we return `Some(converted_anchor_tag, target)`.
///
/// The base path for this conversion (usually where the HTML file resides),
/// is `docdir`.
/// If not `rewrite_rel_links`, relative local links are not converted.
/// Furthermore, all local _absolute_ (not converted) links are prepended with
/// `root_path`. All external URLs always remain untouched.
/// If `rewrite_abs_links` and `link` is absolute, concatenate and return
/// `root_path` and `dest`.
/// If not `rewrite_abs_links` and dest` is absolute, return `dest`.
/// If `rewrite_ext` is true and the link points to a known Tp-Note file
/// extension, then `.html` is appended to the converted link.
/// Remark: The _anchor's text property_ is never changed. However, there is
/// one exception: when the text contains a URL starting with `http:` or
/// `https:`, only the file stem is kept. Example, the anchor text property:
/// `<a ...>http:dir/my file.md</a>` is rewritten into `<a ...>my file</a>`.
///
/// Contracts:
/// 1. `link` must be local (link dest may contain`;///` but not `;//`). It
///    may have a scheme.
/// 2. `link` is `Link::Text2Dest` or `Link::Image`
/// 3. `root_path` and `docdir` are absolute paths to directories.
/// 4. `root_path` is never empty `""`. It can be `"/"`.
///
/// Guaranties:
/// 1. The returned link is guaranteed to be a child of `root_path`, or
///    `None`.

fn rewrite_local_link(
    link: Link,
    root_path: &Path,
    docdir: &Path,
    rewrite_rel_links: bool,
    rewrite_abs_links: bool,
    rewrite_ext: bool,
) -> Option<(String, PathBuf)> {
    //
    match link {
        Link::Text2Dest(text, dest, title) => {
            // Check contract 1. Panic if link is not local.
            debug_assert!(!dest.contains("://") && !dest.contains(":///"));

            // Only rewrite file extensions for Tp-Note files.
            let rewrite_ext =
                rewrite_ext && MarkupLanguage::from(Path::new(dest.as_ref())).is_some();

            // Is this an autolink?
            // Show only the stem as link text.
            let mut text = text;
            if text == dest && (text.contains(':') || text.contains('@')) {
                let short_text = text
                    .trim_start_matches("http://")
                    .trim_start_matches("http:")
                    .trim_start_matches("tpnote:");
                let short_text = Path::new(short_text);
                let short_text = short_text
                    .file_stem()
                    .unwrap_or_default()
                    .to_str()
                    .unwrap_or_default();
                text = Cow::Owned(short_text.to_string());
            }

            // As we have only local destinations here, we trim the URL scheme.
            let short_dest = dest
                .trim_start_matches("http://")
                .trim_start_matches("http:")
                .trim_start_matches("tpnote:");
            let mut dest = if let Cow::Owned(_) = dest {
                Cow::Owned(short_dest.to_string())
            } else {
                Cow::Borrowed(short_dest)
            };

            // Append ".html" if `rewrite_ext`.
            if rewrite_ext {
                let mut long_dest = dest.to_string();
                long_dest.push_str(HTML_EXT);
                dest = Cow::Owned(long_dest)
            };

            let destout = assemble_link(
                root_path,
                docdir,
                Path::new(&dest.as_ref()),
                rewrite_rel_links,
                rewrite_abs_links,
            )?;

            // Replace Windows backslash
            let deststr = destout.to_str().unwrap_or_default();
            let deststr = if deststr.contains('\\') {
                Cow::Owned(deststr.replace('\\', "/"))
            } else {
                Cow::Borrowed(deststr)
            };

            Some((
                format!("<a href=\"{}\" title=\"{}\">{}</a>", deststr, title, text),
                destout,
            ))
        }

        Link::Image(text, dest) => {
            // Check contract 1. Panic if link is not local.
            debug_assert!(!dest.contains("://"));

            let destout = assemble_link(
                root_path,
                docdir,
                Path::new(&dest.as_ref()),
                rewrite_rel_links,
                rewrite_abs_links,
            )?;

            // Replace Windows backslash
            let deststr = destout.to_str().unwrap_or_default();
            let deststr = if deststr.contains('\\') {
                Cow::Owned(deststr.replace('\\', "/"))
            } else {
                Cow::Borrowed(deststr)
            };

            Some((
                format!("<img src=\"{}\" alt=\"{}\" />", deststr, text),
                destout,
            ))
        }

        _ => unreachable!(),
    }
}

/// A helper function, that percent decodes the link destinations (and the link text
/// in case of an autolink). As we insert the result in an UTF-8 template, we
/// can
fn percent_decode(link: Link) -> Link {
    match link {
        Link::Text2Dest(text, dest, title) => {
            // Is this an autolink?
            let autolink = *text == *dest && (text.contains(':') || text.contains('@'));

            // Percent decode URL. The template we insert in is UTF-8 encoded.
            let decoded_dest = percent_decode_str(&dest).decode_utf8().unwrap();
            let dest = match (&dest, &decoded_dest) {
                (Cow::Borrowed(_), Cow::Borrowed(_)) => dest,
                _ => Cow::Owned(decoded_dest.to_string()),
            };

            // We also check `text == decoded(dest)`.
            let text = if autolink || *text == *dest {
                dest.clone()
            } else {
                text
            };

            Link::Text2Dest(text, dest, title)
        }

        Link::Image(alt_text, source) => {
            // Percent decode URL. The template we insert in is UTF-8 encoded.
            let decoded_source = percent_decode_str(&source).decode_utf8().unwrap();
            let source = match (&source, &decoded_source) {
                (Cow::Borrowed(_), Cow::Borrowed(_)) => source,
                _ => Cow::Owned(decoded_source.to_string()),
            };

            Link::Image(alt_text, source)
        }

        // The `HyperlinkInlineImage` iterator has no further variants.
        _ => unreachable!(),
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
    for ((skipped, consumed, remaining), link) in HyperlinkInlineImage::new(&html) {
        //
        // Is this an absolute hyperlink?
        html_out.push_str(skipped);
        rest = remaining;
        {
            let link_destination = match link {
                Link::Text2Dest(ref _link_text, ref link_destination, ref _link_title) => {
                    link_destination
                }
                Link::Image(ref _img_alt, ref img_src) => img_src,
                _ => continue,
            };

            // We skip absolute URLs, `mailto:` and `tel:` links.
            if (link_destination.contains("://") && !link_destination.contains(":///"))
                || link_destination.starts_with("mailto:")
                || link_destination.starts_with("tel:")
            {
                html_out.push_str(consumed);
                continue;
            }
        }

        // From here on we only deal with local links.
        // Percent decode link destination.
        let link = percent_decode(link);

        // Rewrite the local link.
        if let Some((consumed_new, dest)) = rewrite_local_link(
            link,
            root_path,
            docdir,
            rewrite_rel_links,
            rewrite_abs_links,
            rewrite_ext,
        ) {
            html_out.push_str(&consumed_new);
            allowed_urls.insert(dest);
        } else {
            log::debug!("Viewer: invalid_local_links: {}", consumed);
            html_out.push_str("<i>INVALID LOCAL LINK</i>");
        }
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

    use crate::html::assemble_link;
    use crate::html::percent_decode;
    use crate::html::rewrite_links;
    use crate::html::rewrite_local_link;
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
    #[should_panic(expected = "assertion failed: !dest.contains(\\\"://\\\")")]
    fn test_rewrite_link1() {
        let root_path = Path::new("/my/");
        let docdir = Path::new("/my/abs/note path/");

        // Should panic: this is not a relative path.
        let input = take_link("<a href=\"ftp://getreu.net\">Blog</a>")
            .unwrap()
            .1
             .1;
        let _ = rewrite_local_link(input, root_path, docdir, true, false, false).unwrap();
    }

    #[test]
    fn test_rewrite_link2() {
        let root_path = Path::new("/my/");
        let docdir = Path::new("/my/abs/note path/");

        // Check relative path to image.
        let input = take_link("<img src=\"down/./down/../../t m p.jpg\" alt=\"Image\" />")
            .unwrap()
            .1
             .1;
        let expected = "<img src=\"/abs/note path/t m p.jpg\" \
            alt=\"Image\" />";
        let (outhtml, outpath) =
            rewrite_local_link(input, root_path, docdir, true, false, false).unwrap();

        assert_eq!(outhtml, expected);
        assert_eq!(outpath, PathBuf::from("/abs/note path/t m p.jpg"));

        // Check relative path to image. Canonicalized?
        let input = take_link("<img src=\"down/./../../t m p.jpg\" alt=\"Image\" />")
            .unwrap()
            .1
             .1;
        let expected = "<img src=\"../t m p.jpg\" alt=\"Image\" />";
        let (outhtml, outpath) =
            rewrite_local_link(input, root_path, docdir, false, false, false).unwrap();

        assert_eq!(outhtml, expected);
        assert_eq!(outpath, PathBuf::from("../t m p.jpg"));

        // Check relative path to note file.
        let input = take_link("<a href=\"./down/./../my note 1.md\">my note 1</a>")
            .unwrap()
            .1
             .1;
        let expected = "<a href=\"/abs/note path/my note 1.md\" \
            title=\"\">my note 1</a>";
        let (outhtml, outpath) =
            rewrite_local_link(input, root_path, docdir, true, false, false).unwrap();

        assert_eq!(outhtml, expected);
        assert_eq!(outpath, PathBuf::from("/abs/note path/my note 1.md"));

        // Check absolute path to note file.
        let input = take_link("<a href=\"/dir/./down/../my note 1.md\">my note 1</a>")
            .unwrap()
            .1
             .1;
        let expected = "<a href=\"/dir/my note 1.md\" \
            title=\"\">my note 1</a>";
        let (outhtml, outpath) =
            rewrite_local_link(input, root_path, docdir, true, false, false).unwrap();

        assert_eq!(outhtml, expected);
        assert_eq!(outpath, PathBuf::from("/dir/my note 1.md"));

        // Check relative path to note file. Canonicalized?
        let input = take_link("<a href=\"./down/./../dir/my note 1.md\">my note 1</a>")
            .unwrap()
            .1
             .1;
        let expected = "<a href=\"dir/my note 1.md\" \
            title=\"\">my note 1</a>";
        let (outhtml, outpath) =
            rewrite_local_link(input, root_path, docdir, false, false, false).unwrap();

        assert_eq!(outhtml, expected);
        assert_eq!(outpath, PathBuf::from("dir/my note 1.md"));

        // Check `rewrite_ext=true`.
        let input = take_link("<a href=\"./down/./../dir/my note 1.md\">my note 1</a>")
            .unwrap()
            .1
             .1;
        let expected = "<a href=\"/abs/note path/dir/my note 1.md.html\" \
            title=\"\">my note 1</a>";
        let (outhtml, outpath) =
            rewrite_local_link(input, root_path, docdir, true, false, true).unwrap();

        assert_eq!(outhtml, expected);
        assert_eq!(
            outpath,
            PathBuf::from("/abs/note path/dir/my note 1.md.html")
        );

        // Check relative link in input.
        let input = take_link("<a href=\"./down/./../dir/my note 1.md\">my note 1</a>")
            .unwrap()
            .1
             .1;
        let expected = "<a href=\"/path/dir/my note 1.md\" title=\"\">my note 1</a>";
        let (outhtml, outpath) = rewrite_local_link(
            input,
            Path::new("/my/note/"),
            Path::new("/my/note/path/"),
            true,
            false,
            false,
        )
        .unwrap();

        assert_eq!(outhtml, expected);
        assert_eq!(outpath, PathBuf::from("/path/dir/my note 1.md"));

        // Check absolute link in input.
        let input = take_link("<a href=\"/down/./../dir/my note 1.md\">my note 1</a>")
            .unwrap()
            .1
             .1;
        let expected = "<a href=\"/dir/my note 1.md\" title=\"\">my note 1</a>";
        let (outhtml, outpath) = rewrite_local_link(
            input,
            root_path,
            Path::new("/my/ignored/"),
            true,
            false,
            false,
        )
        .unwrap();

        assert_eq!(outhtml, expected);
        assert_eq!(outpath, PathBuf::from("/dir/my note 1.md"));

        // Check absolute link in input, not in `root_path`.
        let input = take_link("<a href=\"/down/../../dir/my note 1.md\">my note 1</a>")
            .unwrap()
            .1
             .1;
        let output = rewrite_local_link(
            input,
            root_path,
            Path::new("/my/notepath/"),
            true,
            false,
            false,
        );

        assert_eq!(output, None);

        // Check relative link in input, not in `root_path`.
        let input = take_link("<a href=\"../../dir/my note 1.md\">my note 1</a>")
            .unwrap()
            .1
             .1;
        let output = rewrite_local_link(
            input,
            root_path,
            Path::new("/my/notepath/"),
            true,
            false,
            false,
        );

        assert_eq!(output, None);

        // Check relative link in input, with underflow.
        let root_path = Path::new("/");
        let input = take_link("<a href=\"../../dir/my note 1.md\">my note 1</a>")
            .unwrap()
            .1
             .1;
        let output = rewrite_local_link(input, root_path, Path::new("/my/"), true, false, false);

        assert_eq!(output, None);

        // Check relative link in input, not in `root_path`.
        let root_path = Path::new("/my");
        let input = take_link("<a href=\"../../dir/my note 1.md\">my note 1</a>")
            .unwrap()
            .1
             .1;
        let output = rewrite_local_link(
            input,
            root_path,
            Path::new("/my/notepath"),
            true,
            false,
            false,
        );

        assert_eq!(output, None);
    }

    #[test]
    fn test_percent_decode() {
        //
        let input = Link::Text2Dest(Cow::from("text"), Cow::from("dest"), Cow::from("title"));
        let expected = Link::Text2Dest(Cow::from("text"), Cow::from("dest"), Cow::from("title"));
        assert_eq!(percent_decode(input), expected);

        //
        let input = Link::Text2Dest(
            Cow::from("te%20xt"),
            Cow::from("de%20st"),
            Cow::from("title"),
        );
        let expected =
            Link::Text2Dest(Cow::from("te%20xt"), Cow::from("de st"), Cow::from("title"));
        assert_eq!(percent_decode(input), expected);

        //
        let input = Link::Text2Dest(
            Cow::from("de%20st"),
            Cow::from("de%20st"),
            Cow::from("title"),
        );
        let expected =
            Link::Text2Dest(Cow::from("de%20st"), Cow::from("de st"), Cow::from("title"));
        assert_eq!(percent_decode(input), expected);

        //
        let input = Link::Text2Dest(
            Cow::from("d:e%20st"),
            Cow::from("d:e%20st"),
            Cow::from("title"),
        );
        let expected =
            Link::Text2Dest(Cow::from("d:e st"), Cow::from("d:e st"), Cow::from("title"));
        assert_eq!(percent_decode(input), expected);

        //
        let input = Link::Image(Cow::from("al%20t"), Cow::from("de%20st"));
        let expected = Link::Image(Cow::from("al%20t"), Cow::from("de st"));
        assert_eq!(percent_decode(input), expected);

        //
        let input = Link::Image(Cow::from("a\\lt"), Cow::from("d\\est"));
        let expected = Link::Image(Cow::from("a\\lt"), Cow::from("d\\est"));
        assert_eq!(percent_decode(input), expected);
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
            jkl<a href=\"/abs/note path/down/my note 1.md\" title=\"\">my note 1</a>\
            mno<a href=\"/abs/note path/dir/my note.md\" title=\"\">my note</a>\
            pqr<a href=\"/dir/my note.md\" title=\"\">my note</a>\
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
