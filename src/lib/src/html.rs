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

#[inline]
/// Helper function that converts relative local HTML links to absolute
/// links. If successful, we return `Some(converted_anchor_tag, target)`. If
/// `prepend_dirpath` is empty, no convertion is perfomed. Local absolute links
/// and external URLs always remain untouched. If `rewrite_ext` is true and
/// the link points to a known Tp-Note file extension, then `.html` is appended
/// to the converted link. In case of error, we return `None`.
/// Remark: The _anchor's text property_ is never changed. However, there is
/// one exception: when the text contains a URL starting with `http:` or
/// `https:`, only the file stem is kept. Example, the anchor text property:
/// `<a ...>http:dir/my file.md</a>` is rewritten into `<a ...>my file</a>`.
/// Contract: Links must be local. They may have a scheme.
fn rel_link_to_abs_link(
    link: &str,
    prepend_dirpath: &Path,
    rewrite_ext: bool,
) -> Option<(String, PathBuf)> {
    //
    const ASCIISET: percent_encoding::AsciiSet = NON_ALPHANUMERIC
        .remove(b'/')
        .remove(b'.')
        .remove(b'_')
        .remove(b'-');

    let mut dirpath_link = prepend_dirpath.to_owned();

    match take_link(link) {
        Ok(("", ("", Link::Text2Dest(text, dest, title)))) => {
            // Check contract. Panic if link is not local.
            debug_assert!(!link.contains("://"));

            // Only rewrite file extensions for Tp-Note files.
            let rewrite_ext = rewrite_ext
                && !matches!(
                    MarkupLanguage::from(Path::new(&*dest)),
                    MarkupLanguage::None
                );

            // Local ones are ok. Trim URL scheme.
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

            // Concat `abspath` and `relpath`.
            let relpath_link = &*percent_decode_str(dest).decode_utf8().unwrap();

            // Append ".html" if `rewrite_ext`.
            let relpath_link = if rewrite_ext {
                let mut relpath_link = relpath_link.to_string();
                relpath_link.push_str(HTML_EXT);
                PathBuf::from(relpath_link)
            } else {
                PathBuf::from(relpath_link)
            };

            // If `dirpath_link` is empty, use relpath_link instead.
            if dirpath_link.to_str().unwrap_or_default().is_empty() {
                dirpath_link = relpath_link;
            } else {
                for p in relpath_link.iter() {
                    if p == "." {
                        continue;
                    }
                    if p == ".." {
                        dirpath_link.pop();
                    } else {
                        dirpath_link.push(p);
                    }
                }
            }

            let abspath_link_encoded =
                utf8_percent_encode(dirpath_link.to_str().unwrap_or_default(), &ASCIISET)
                    .to_string();
            Some((
                format!("<a href=\"{abspath_link_encoded}\" title=\"{title}\">{short_text}</a>"),
                dirpath_link,
            ))
        }

        Ok(("", ("", Link::Image(text, dest)))) => {
            // Concat `abspath` and `relpath`.
            let relpath_link = PathBuf::from(&*percent_decode_str(&dest).decode_utf8().unwrap());

            // If `dirpath_link` is empty, use relpath_link instead.
            if dirpath_link.to_str().unwrap_or_default().is_empty() {
                dirpath_link = relpath_link;
            } else {
                for p in relpath_link.iter() {
                    if p == "." {
                        continue;
                    }
                    if p == ".." {
                        dirpath_link.pop();
                    } else {
                        dirpath_link.push(p);
                    }
                }
            }

            let abspath_link_encoded =
                utf8_percent_encode(dirpath_link.to_str().unwrap_or_default(), &ASCIISET)
                    .to_string();
            Some((
                format!("<img src=\"{abspath_link_encoded}\" alt=\"{text}\" />"),
                dirpath_link,
            ))
        }
        Ok((_, (_, _))) | Err(_) => None,
    }
}

#[inline]
/// Helper function that scans the input `html` string and converts
/// all relative local HTML links to absolute links by prepending
/// `prepend_dirpath`. If `prepend_dirpath` is empty, no convertion is
/// perfomed. Local absolute links and external URLs always remain untouched.
/// If `rewrite_ext` is true and  the link points to a known Tp-Note file
/// extension, then `.html` is appended to the converted link. The resulting
/// HTML string contains all rewritten links, whose targets are finally added
/// to the `allowed_urls`.
/// Remark: The _anchor's text property_ is never changed. However, there is
/// one exception: when the text contains a URL starting with `http:` or
/// `https:`, only the file stem is kept. Example, the anchor text property:
/// `<a ...>http:dir/my file.md</a>` is rewritten into `<a ...>my file</a>`.
pub fn rewrite_links(
    html: String,
    prepend_dirpath: &Path,
    rewrite_ext: bool,
    allowed_urls: Arc<RwLock<HashSet<PathBuf>>>,
) -> String {
    let mut allowed_urls = allowed_urls
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

        if let Some((consumed_new, url)) =
            rel_link_to_abs_link(consumed, prepend_dirpath, rewrite_ext)
        {
            html_out.push_str(&consumed_new);
            allowed_urls.insert(url);
        } else {
            log::debug!("Viewer: can not parse URL: {}", consumed);
            html_out.push_str("<i>INVALID URL</i>");
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
                    s.push_str(p.as_path().to_str().unwrap_or_default());
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

    use crate::html::rel_link_to_abs_link;
    use crate::html::rewrite_links;

    #[test]
    #[should_panic(expected = "assertion failed: !link.contains(\\\"://\\\")")]
    fn test_rel_link_to_abs_link1() {
        let absdir = Path::new("/my/abs/note path/");

        // Should panic: this is not a relative path.
        let input = "<a href=\"ftp://getreu.net\">Blog</a>";
        let _ = rel_link_to_abs_link(input, absdir, false).unwrap();
    }

    #[test]
    fn test_rel_link_to_abs_link2() {
        // Check relative path to image.
        let absdir = Path::new("/my/abs/note path/");

        let input = "<img src=\"down/./down/../../t%20m%20p.jpg\" alt=\"Image\" />";
        let expected = "<img src=\"/my/abs/note%20path/t%20m%20p.jpg\" \
            alt=\"Image\" />";
        let (outhtml, outpath) = rel_link_to_abs_link(input, absdir, false).unwrap();

        assert_eq!(outhtml, expected);
        assert_eq!(outpath, PathBuf::from("/my/abs/note path/t m p.jpg"));

        // Check relative path to image, with empty prepend path.
        let input = "<img src=\"down/./../../t%20m%20p.jpg\" alt=\"Image\" />";
        let expected = "<img src=\"down/./../../t%20m%20p.jpg\" alt=\"Image\" />";
        let (outhtml, outpath) = rel_link_to_abs_link(input, Path::new(""), false).unwrap();

        assert_eq!(outhtml, expected);
        assert_eq!(outpath, PathBuf::from("down/./../../t m p.jpg"));

        // Check relative path to note file.
        let input = "<a href=\"./down/./../my%20note%201.md\">my note 1</a>";
        let expected = "<a href=\"/my/abs/note%20path/my%20note%201.md\" \
            title=\"\">my note 1</a>";
        let (outhtml, outpath) = rel_link_to_abs_link(input, absdir, false).unwrap();

        assert_eq!(outhtml, expected);
        assert_eq!(outpath, PathBuf::from("/my/abs/note path/my note 1.md"));

        // Check absolute path to note file.
        let input = "<a href=\"/dir/./down/../my%20note%201.md\">my note 1</a>";
        let expected = "<a href=\"/dir/my%20note%201.md\" \
            title=\"\">my note 1</a>";
        let (outhtml, outpath) = rel_link_to_abs_link(input, absdir, false).unwrap();

        assert_eq!(outhtml, expected);
        assert_eq!(outpath, PathBuf::from("/dir/my note 1.md"));

        // Check relative path to note file, with `./` for prepend path.
        let input = "<a href=\"./down/./../dir/my%20note%201.md\">my note 1</a>";
        let expected = "<a href=\"./dir/my%20note%201.md\" \
            title=\"\">my note 1</a>";
        let (outhtml, outpath) = rel_link_to_abs_link(input, Path::new("./"), false).unwrap();

        assert_eq!(outhtml, expected);
        assert_eq!(outpath, PathBuf::from("./dir/my note 1.md"));

        // Check `rewrite_ext=true`.
        let input = "<a href=\"./down/./../dir/my%20note%201.md\">my note 1</a>";
        let expected = "<a href=\"./dir/my%20note%201.md.html\" \
            title=\"\">my note 1</a>";
        let (outhtml, outpath) = rel_link_to_abs_link(input, Path::new("./"), true).unwrap();

        assert_eq!(outhtml, expected);
        assert_eq!(outpath, PathBuf::from("./dir/my note 1.md.html"));

        // Check relative path to note file, with empty prepend path.
        let input = "<a href=\"./down/./../dir/my%20note%201.md\">my note 1</a>";
        let expected = "<a href=\"./down/./../dir/my%20note%201.md\" title=\"\">my note 1</a>";
        let (outhtml, outpath) = rel_link_to_abs_link(input, Path::new(""), false).unwrap();

        assert_eq!(outhtml, expected);
        assert_eq!(outpath, PathBuf::from("./down/./../dir/my note 1.md"));

        // Check relative path to note file, with `/` prepend path.
        let input = "<a href=\"./down/./../dir/my%20note%201.md\">my note 1</a>";
        let expected = "<a href=\"/dir/my%20note%201.md\" title=\"\">my note 1</a>";
        let (outhtml, outpath) = rel_link_to_abs_link(input, Path::new("/"), false).unwrap();

        assert_eq!(outhtml, expected);
        assert_eq!(outpath, PathBuf::from("/dir/my note 1.md"));
    }

    #[test]
    fn test_rewrite_abs_links() {
        let allowed_urls = Arc::new(RwLock::new(HashSet::new()));
        let input = "abc<a href=\"ftp://getreu.net\">Blog</a>\
            def<a href=\"https://getreu.net\">https://getreu.net</a>\
            ghi<img src=\"t%20m%20p.jpg\" alt=\"test 1\" />\
            jkl<a href=\"down/../down/my%20note%201.md\">my note 1</a>\
            mno<a href=\"http:./down/../dir/my%20note.md\">\
            http:./down/../dir/my%20note.md</a>"
            .to_string();
        let absdir = Path::new("/my/abs/note path/");
        let expected = "abc<a href=\"ftp://getreu.net\">Blog</a>\
            def<a href=\"https://getreu.net\">https://getreu.net</a>\
            ghi<img src=\"/my/abs/note%20path/t%20m%20p.jpg\" alt=\"test 1\" />\
            jkl<a href=\"/my/abs/note%20path/down/my%20note%201.md\" title=\"\">my note 1</a>\
            mno<a href=\"/my/abs/note%20path/dir/my%20note.md\" title=\"\">my note</a>"
            .to_string();

        let output = rewrite_links(input, absdir, false, allowed_urls.clone());
        let url = allowed_urls.read().unwrap();

        assert_eq!(output, expected);
        assert!(url.contains(&PathBuf::from("/my/abs/note path/t m p.jpg")));
        assert!(url.contains(&PathBuf::from("/my/abs/note path/dir/my note.md")));
        assert!(url.contains(&PathBuf::from("/my/abs/note path/down/my note 1.md")));
    }
}
