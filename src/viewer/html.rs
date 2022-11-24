//! Helper functions dealing with HTML conversion.

use parse_hyperlinks::parser::Link;
use parse_hyperlinks_extras::iterator_html::HyperlinkInlineImage;
use parse_hyperlinks_extras::parser::parse_html::take_link;
use percent_encoding::{percent_decode_str, utf8_percent_encode, NON_ALPHANUMERIC};
use std::{
    collections::HashSet,
    path::{Path, PathBuf},
    sync::{Arc, RwLock},
};

#[inline]
/// Helper function that converts relative local HTML link to absolute
/// local HTML link. If successful, it returns `Some(converted_anchor_tag, target)`.
/// In case of error, it returns `None`.
/// Contract: Links must be local. They may have a scheme.
fn rel_link_to_abs_link(link: &str, abspath_dir: &Path) -> Option<(String, PathBuf)> {
    //
    const ASCIISET: percent_encoding::AsciiSet = NON_ALPHANUMERIC
        .remove(b'/')
        .remove(b'.')
        .remove(b'_')
        .remove(b'-');

    let mut abspath_link = abspath_dir.to_owned();

    match take_link(link) {
        Ok(("", ("", Link::Text2Dest(text, dest, title)))) => {
            // Check contract. Panic if link is not local.
            debug_assert!(!link.contains("://"));

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
            let relpath_link = PathBuf::from(&*percent_decode_str(dest).decode_utf8().unwrap());
            for p in relpath_link.iter() {
                if p == "." {
                    continue;
                }
                if p == ".." {
                    abspath_link.pop();
                } else {
                    abspath_link.push(p);
                }
            }
            let abspath_link_encoded =
                utf8_percent_encode(abspath_link.to_str().unwrap_or_default(), &ASCIISET)
                    .to_string();
            Some((
                format!("<a href=\"{abspath_link_encoded}\" title=\"{title}\">{short_text}</a>"),
                abspath_link,
            ))
        }

        Ok(("", ("", Link::Image(text, dest)))) => {
            // Concat `abspath` and `relpath`.
            let relpath_link = PathBuf::from(&*percent_decode_str(&dest).decode_utf8().unwrap());
            for p in relpath_link.iter() {
                if p == "." {
                    continue;
                }
                if p == ".." {
                    abspath_link.pop();
                } else {
                    abspath_link.push(p);
                }
            }
            let abspath_link_encoded =
                utf8_percent_encode(abspath_link.to_str().unwrap_or_default(), &ASCIISET)
                    .to_string();
            Some((
                format!("<img src=\"{abspath_link_encoded}\" alt=\"{text}\">"),
                abspath_link,
            ))
        }
        Ok((_, (_, _))) | Err(_) => None,
    }
}

#[inline]
/// Helper function that scans the input `html` and converts all relative
/// local HTML links to absolute local HTML links. The absolute links are
/// added to `allowed_urls`.
pub(crate) fn rewrite_links(
    html: String,
    abspath_dir: &Path,
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

        if let Some((consumed_new, url)) = rel_link_to_abs_link(consumed, abspath_dir) {
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

    use crate::viewer::html::rel_link_to_abs_link;
    use crate::viewer::html::rewrite_links;

    #[test]
    #[should_panic(expected = "assertion failed: !link.contains(\\\"://\\\")")]
    fn test_rel_link_to_abs_link1() {
        let absdir = Path::new("/my/abs/note path/");

        // Should panic: this is not a relative path.
        let input = "<a href=\"ftp://getreu.net\">Blog</a>";
        let _ = rel_link_to_abs_link(input, absdir).unwrap();
    }

    #[test]
    fn test_rel_link_to_abs_link2() {
        // Check relative path to image.
        let absdir = Path::new("/my/abs/note path/");

        let input = "<img src=\"down/./down/../../t%20m%20p.jpg\" alt=\"Image\" />";
        let expected = "<img src=\"/my/abs/note%20path/t%20m%20p.jpg\" \
            alt=\"Image\">";
        let (outhtml, outpath) = rel_link_to_abs_link(input, absdir).unwrap();

        assert_eq!(outhtml, expected);
        assert_eq!(outpath, PathBuf::from("/my/abs/note path/t m p.jpg"));

        // Check relative path to note file.
        let input = "<a href=\"./down/./../my%20note%201.md\">my note 1</a>";
        let expected = "<a href=\"/my/abs/note%20path/my%20note%201.md\" \
            title=\"\">my note 1</a>";
        let (outhtml, outpath) = rel_link_to_abs_link(input, absdir).unwrap();

        assert_eq!(outhtml, expected);
        assert_eq!(outpath, PathBuf::from("/my/abs/note path/my note 1.md"));
    }

    #[test]
    fn test_rewrite_links1() {
        let allowed_urls = Arc::new(RwLock::new(HashSet::new()));
        let input = "abc<a href=\"/down/../down/my%20note%201.md\">my note 1</a>efg".to_string();
        let absdir = Path::new("/my/abs/note path/");
        let expected = "abc<i>INVALID URL</i>efg".to_string();

        let output = rewrite_links(input, absdir, allowed_urls.clone());
        let url = allowed_urls.read().unwrap();

        assert_eq!(output, expected);
        assert!(url.is_empty());
    }

    #[test]
    fn test_rewrite_abs_links2() {
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
            ghi<img src=\"/my/abs/note%20path/t%20m%20p.jpg\" alt=\"test 1\">\
            jkl<a href=\"/my/abs/note%20path/down/my%20note%201.md\" title=\"\">my note 1</a>\
            mno<a href=\"/my/abs/note%20path/dir/my%20note.md\" title=\"\">my note</a>"
            .to_string();

        let output = rewrite_links(input, absdir, allowed_urls.clone());
        let url = allowed_urls.read().unwrap();

        assert_eq!(output, expected);
        assert!(url.contains(&PathBuf::from("/my/abs/note path/t m p.jpg")));
        assert!(url.contains(&PathBuf::from("/my/abs/note path/dir/my note.md")));
        assert!(url.contains(&PathBuf::from("/my/abs/note path/down/my note 1.md")));
    }
}
