//! Helper functions dealing with HTML conversion.

use crate::markup_language::MarkupLanguage;
use parse_hyperlinks::parser::Link;
use parse_hyperlinks_extras::iterator_html::HyperlinkInlineImage;
use parse_hyperlinks_extras::parser::parse_html::take_link;
use percent_encoding::{percent_decode_str, utf8_percent_encode, NON_ALPHANUMERIC};
use std::{
    collections::HashSet,
    path::{Path, PathBuf},
    sync::{Arc, RwLock},
};

pub const HTML_EXT: &str = ".html";

/// If `rewrite_rel_links` and `dest` is relative, concat `docdir`  and
/// `dest`, then strip `root_path` from the left before returning.
/// If not `rewrite_rel_links` and `dest` is relative, return `dest`.
/// If `dest` is absolute  and return `dest`.
/// The `dest` portion of the output is always canonicalized.
/// Return the assembled path, when in `root_path`, or `None` otherwise.
/// Asserts in debug mode, that `doc_dir` is in `root_path`.
fn assemble_link(
    root_path: &Path,
    docdir: &Path,
    dest: &Path,
    rewrite_rel_links: bool,
) -> Option<PathBuf> {
    ///
    /// Concatenate `path` and `append`.
    /// The `append` portion of the output is always canonicalized.
    /// In case of underflow, returned path starts with `/..`.
    fn append(path: &mut PathBuf, append: &Path) {
        // Append `dest` to `link` and canonicalize.
        for dir in append.iter() {
            // `/` filtered because it resets the path.
            if dir == "." || dir == "/" {
                continue;
            }
            if dir == ".." {
                if !path.pop() {
                    path.push(dir);
                };
            } else {
                path.push(dir);
            }
        }
    }

    //
    debug_assert!(docdir.starts_with(root_path));
    // Check if the link points into `root_path`, reject otherwise.
    let mut abslink = if dest.is_relative() {
        docdir.to_path_buf()
    } else {
        root_path.to_path_buf()
    };
    append(&mut abslink, dest);

    if !abslink.starts_with(root_path) || abslink.starts_with("/..") {
        return None;
    };

    // Caculate the output.
    let mut link = match (rewrite_rel_links, dest.is_relative()) {
        (true, true) => {
            // Result: "/" + docdir.strip(root_path) + dest
            let link = PathBuf::from("/");
            link.join(docdir.strip_prefix(root_path).ok()?)
        }
        // Result: dest
        (false, true) => PathBuf::new(),
        // Result: "/" + dest.strip(root_path)
        (_, false) => PathBuf::from("/"),
    };
    append(&mut link, dest);

    Some(link)
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
/// If `rewrite_ext` is true and the link points to a known Tp-Note file
/// extension, then `.html` is appended to the converted link.
/// Remark: The _anchor's text property_ is never changed. However, there is
/// one exception: when the text contains a URL starting with `http:` or
/// `https:`, only the file stem is kept. Example, the anchor text property:
/// `<a ...>http:dir/my file.md</a>` is rewritten into `<a ...>my file</a>`.
///
/// Contract 1: `link` must be local. It may have a scheme.
/// Contract 2: `root_path` and `docdir` are absolute paths to directories.
/// Contract 3: `root_path` is never empty `""`. It can be `"/"`.
/// Contract 4: The returned link is guaranteed to be a child of `root_path`, or
/// `None`.

fn rewrite_link(
    link: &str,
    root_path: &Path,
    docdir: &Path,
    rewrite_rel_links: bool,
    rewrite_ext: bool,
) -> Option<(String, PathBuf)> {
    //
    const ASCIISET: percent_encoding::AsciiSet = NON_ALPHANUMERIC
        .remove(b'/')
        .remove(b'.')
        .remove(b'_')
        .remove(b'-');

    match take_link(link) {
        Ok(("", ("", Link::Text2Dest(text, dest, title)))) => {
            // Check contract 1. Panic if link is not local.
            debug_assert!(!link.contains("://"));

            // Only rewrite file extensions for Tp-Note files.
            let rewrite_ext = rewrite_ext
                && !matches!(
                    MarkupLanguage::from(Path::new(&*dest)),
                    MarkupLanguage::None
                );

            // Local ones are Ok. Trim URL scheme.
            let dest = dest
                .trim_start_matches("http:")
                .trim_start_matches("https:");

            let mut short_text = text.to_string();

            // Example: display `my text` for the local relative URL: `<http:my%20text.md>`.
            if text.starts_with("http:") || text.starts_with("https:") {
                // Improves pretty printing:
                let text = text
                    .trim_start_matches("http:")
                    .trim_start_matches("https:");
                let text = PathBuf::from(&*percent_decode_str(text).decode_utf8().unwrap());
                let text = text
                    .file_stem()
                    .unwrap_or_default()
                    .to_str()
                    .unwrap_or_default();
                short_text = text.to_string();
            }

            // Append ".html" if `rewrite_ext`.
            let dest = &*percent_decode_str(dest).decode_utf8().unwrap();
            let dest = if rewrite_ext {
                let mut dest = dest.to_string();
                dest.push_str(HTML_EXT);
                PathBuf::from(dest)
            } else {
                PathBuf::from(dest)
            };

            let destout = assemble_link(root_path, docdir, &dest, rewrite_rel_links)?;

            let destout_encoded =
                utf8_percent_encode(destout.to_str().unwrap_or_default(), &ASCIISET).to_string();
            Some((
                format!(
                    "<a href=\"{}\" title=\"{}\">{}</a>",
                    destout_encoded, title, short_text
                ),
                destout,
            ))
        }

        Ok(("", ("", Link::Image(text, dest)))) => {
            // Concat `abspath` and `relpath`.
            let dest = PathBuf::from(&*percent_decode_str(&dest).decode_utf8().unwrap());

            let destout = assemble_link(root_path, docdir, &dest, rewrite_rel_links)?;

            let destout_encoded =
                utf8_percent_encode(destout.to_str().unwrap_or_default(), &ASCIISET).to_string();
            Some((
                format!("<img src=\"{}\" alt=\"{}\" />", destout_encoded, text),
                destout,
            ))
        }

        Ok((_, (_, _))) | Err(_) => None,
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
pub fn rewrite_links(
    html: String,
    root_path: &Path,
    docdir: &Path,
    rewrite_rel_links: bool,
    rewrite_ext: bool,
    allowed_local_links: Arc<RwLock<HashSet<PathBuf>>>,
) -> String {
    let mut allowed_urls = allowed_local_links
        .write()
        .expect("Can not write `allowed_urls`. RwLock is poisoned. Panic.");

    // Search for hyperlinks and inline images in the HTML rendition
    // of this note.
    let mut rest = &*html;
    let mut html_out = String::new();
    for ((skipped, consumed, remaining), link) in HyperlinkInlineImage::new(&html) {
        html_out.push_str(skipped);
        rest = remaining;

        // We skip absolute URLs.
        if link.contains("://") {
            html_out.push_str(consumed);
            continue;
        }

        if let Some((consumed_new, dest)) =
            rewrite_link(consumed, root_path, docdir, rewrite_rel_links, rewrite_ext)
        {
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

    use std::{
        collections::HashSet,
        path::{Path, PathBuf},
        sync::{Arc, RwLock},
    };

    use crate::html::rewrite_link;
    use crate::html::rewrite_links;

    #[test]
    #[should_panic(expected = "assertion failed: !link.contains(\\\"://\\\")")]
    fn test_rewrite_link1() {
        let root_path = Path::new("/my/");
        let docdir = Path::new("/my/abs/note path/");

        // Should panic: this is not a relative path.
        let input = "<a href=\"ftp://getreu.net\">Blog</a>";
        let _ = rewrite_link(input, root_path, docdir, true, false).unwrap();
    }

    #[test]
    fn test_rewrite_link2() {
        let root_path = Path::new("/my/");
        let doc_path = Path::new("/my/abs/note path/");

        // Check relative path to image.
        let input = "<img src=\"down/./down/../../t%20m%20p.jpg\" alt=\"Image\" />";
        let expected = "<img src=\"/abs/note%20path/t%20m%20p.jpg\" \
            alt=\"Image\" />";
        let (outhtml, outpath) = rewrite_link(input, root_path, doc_path, true, false).unwrap();

        assert_eq!(outhtml, expected);
        assert_eq!(outpath, PathBuf::from("/abs/note path/t m p.jpg"));

        // Check relative path to image. Canonicalized?
        let input = "<img src=\"down/./../../t%20m%20p.jpg\" alt=\"Image\" />";
        let expected = "<img src=\"../t%20m%20p.jpg\" alt=\"Image\" />";
        let (outhtml, outpath) = rewrite_link(input, root_path, doc_path, false, false).unwrap();

        assert_eq!(outhtml, expected);
        assert_eq!(outpath, PathBuf::from("../t m p.jpg"));

        // Check relative path to note file.
        let input = "<a href=\"./down/./../my%20note%201.md\">my note 1</a>";
        let expected = "<a href=\"/abs/note%20path/my%20note%201.md\" \
            title=\"\">my note 1</a>";
        let (outhtml, outpath) = rewrite_link(input, root_path, doc_path, true, false).unwrap();

        assert_eq!(outhtml, expected);
        assert_eq!(outpath, PathBuf::from("/abs/note path/my note 1.md"));

        // Check absolute path to note file.
        let input = "<a href=\"/dir/./down/../my%20note%201.md\">my note 1</a>";
        let expected = "<a href=\"/dir/my%20note%201.md\" \
            title=\"\">my note 1</a>";
        let (outhtml, outpath) = rewrite_link(input, root_path, doc_path, true, false).unwrap();

        assert_eq!(outhtml, expected);
        assert_eq!(outpath, PathBuf::from("/dir/my note 1.md"));

        // Check relative path to note file. Canonicalized?
        let input = "<a href=\"./down/./../dir/my%20note%201.md\">my note 1</a>";
        let expected = "<a href=\"dir/my%20note%201.md\" \
            title=\"\">my note 1</a>";
        let (outhtml, outpath) = rewrite_link(input, root_path, doc_path, false, false).unwrap();

        assert_eq!(outhtml, expected);
        assert_eq!(outpath, PathBuf::from("dir/my note 1.md"));

        // Check `rewrite_ext=true`.
        let input = "<a href=\"./down/./../dir/my%20note%201.md\">my note 1</a>";
        let expected = "<a href=\"/abs/note%20path/dir/my%20note%201.md.html\" \
            title=\"\">my note 1</a>";
        let (outhtml, outpath) = rewrite_link(input, root_path, doc_path, true, true).unwrap();

        assert_eq!(outhtml, expected);
        assert_eq!(
            outpath,
            PathBuf::from("/abs/note path/dir/my note 1.md.html")
        );

        // Check relative link in input.
        let input = "<a href=\"./down/./../dir/my%20note%201.md\">my note 1</a>";
        let expected = "<a href=\"/path/dir/my%20note%201.md\" title=\"\">my note 1</a>";
        let (outhtml, outpath) = rewrite_link(
            input,
            Path::new("/my/note/"),
            Path::new("/my/note/path/"),
            true,
            false,
        )
        .unwrap();

        assert_eq!(outhtml, expected);
        assert_eq!(outpath, PathBuf::from("/path/dir/my note 1.md"));

        // Check absolute link in input.
        let input = "<a href=\"/down/./../dir/my%20note%201.md\">my note 1</a>";
        let expected = "<a href=\"/dir/my%20note%201.md\" title=\"\">my note 1</a>";
        let (outhtml, outpath) =
            rewrite_link(input, root_path, Path::new("/my/ignored/"), true, false).unwrap();

        assert_eq!(outhtml, expected);
        assert_eq!(outpath, PathBuf::from("/dir/my note 1.md"));

        // Check absolute link in input, not in `root_path`.
        let input = "<a href=\"/down/../../dir/my%20note%201.md\">my note 1</a>";
        let output = rewrite_link(input, root_path, Path::new("/my/notepath/"), true, false);

        assert_eq!(output, None);

        // Check relative link in input, not in `root_path`.
        let input = "<a href=\"../../dir/my%20note%201.md\">my note 1</a>";
        let output = rewrite_link(input, root_path, Path::new("/my/notepath/"), true, false);

        assert_eq!(output, None);

        // Check relative link in input, with underflow.
        let root_path = Path::new("/");
        let input = "<a href=\"../../dir/my%20note%201.md\">my note 1</a>";
        let output = rewrite_link(input, root_path, Path::new("/my/"), true, false);

        assert_eq!(output, None);

        // Check relative link in input, not in `root_path`.
        let root_path = Path::new("/my");
        let input = "<a href=\"../../dir/my%20note%201.md\">my note 1</a>";
        let output = rewrite_link(input, root_path, Path::new("/my/notepath"), true, false);

        assert_eq!(output, None);
    }

    #[test]
    fn test_rewrite_abs_links() {
        let allowed_urls = Arc::new(RwLock::new(HashSet::new()));
        let input = "abc<a href=\"ftp://getreu.net\">Blog</a>\
            def<a href=\"https://getreu.net\">https://getreu.net</a>\
            ghi<img src=\"t%20m%20p.jpg\" alt=\"test 1\" />\
            jkl<a href=\"down/../down/my%20note%201.md\">my note 1</a>\
            mno<a href=\"http:./down/../dir/my%20note.md\">\
            http:./down/../dir/my%20note.md</a>\
            pqr<a href=\"http:/down/../dir/my%20note.md\">\
            http:./down/../dir/my%20note.md</a>\
            stu<a href=\"http:/../dir/underflow/my%20note.md\">\
            not allowed dir</a>\
            vwx<a href=\"http:../../../not allowed dir/my%20note.md\">\
            not allowed</a>"
            .to_string();
        let expected = "abc<a href=\"ftp://getreu.net\">Blog</a>\
            def<a href=\"https://getreu.net\">https://getreu.net</a>\
            ghi<img src=\"/abs/note%20path/t%20m%20p.jpg\" alt=\"test 1\" />\
            jkl<a href=\"/abs/note%20path/down/my%20note%201.md\" title=\"\">my note 1</a>\
            mno<a href=\"/abs/note%20path/dir/my%20note.md\" title=\"\">my note</a>\
            pqr<a href=\"/dir/my%20note.md\" title=\"\">my note</a>\
            stu<i>INVALID LOCAL LINK</i>\
            vwx<i>INVALID LOCAL LINK</i>"
            .to_string();

        let root_path = Path::new("/my/");
        let docdir = Path::new("/my/abs/note path/");
        let output = rewrite_links(input, root_path, docdir, true, false, allowed_urls.clone());
        let url = allowed_urls.read().unwrap();

        assert_eq!(output, expected);
        assert!(url.contains(&PathBuf::from("/abs/note path/t m p.jpg")));
        assert!(url.contains(&PathBuf::from("/abs/note path/dir/my note.md")));
        assert!(url.contains(&PathBuf::from("/abs/note path/down/my note 1.md")));
    }
}
