//! This module abstracts the HTML to Markdown filter.
use crate::error::NoteError;
use html2md::parse_html;

/*
// Alternative implementation:
/// Abstracts the HTML to Markdown conversion.
/// This implementation uses the `htmd` crate.
#[inline]
pub(crate) fn convert_html_to_md(html: &str) -> Result<String, NoteError> {
    use htmd;
    let converter = htmd::HtmlToMarkdown::builder()
        .skip_tags(vec!["script", "style"])
        .build();
    converter.convert(&s).map_err(|e| NoteError::InvalidHtml {
        source_str: e.to_string(),
    })
}
*/

/// Abstracts the HTML to Markdown conversion.
/// This implementation uses the `html2md` crate.
#[inline]
pub(crate) fn convert_html_to_md(html: &str) -> Result<String, NoteError> {
    Ok(parse_html(html))
}

#[cfg(test)]
mod tests {

    use crate::html2md::convert_html_to_md;

    #[test]
    fn test_convert_html_to_md() {
        let input: &str =
            "<div id=\"videopodcast\">outside <span id=\"pills\">inside</span>\n</div>";
        let expected: &str = "outside inside";

        let result = convert_html_to_md(input);
        assert_eq!(result.unwrap(), expected);

        //
        let input: &str = r#"<p><a href="/my_uri">link</a></p>"#;
        let expected: &str = "[link](/my_uri)";

        let result = convert_html_to_md(input);
        assert_eq!(result.unwrap(), expected);

        //
        // [CommonMark: Example 489](https://spec.commonmark.org/0.31.2/#example-489)
        let input: &str = r#"<p><a href="/my uri">link</a></p>"#;
        let expected: &str = "[link](</my uri>)";

        let result = convert_html_to_md(input);
        assert_eq!(result.unwrap(), expected);

        //
        // [CommonMark: Example 489](https://spec.commonmark.org/0.31.2/#example-489)
        let input: &str = r#"<p><a href="/my%20uri">link</a></p>"#;
        let expected: &str = "[link](</my uri>)";

        let result = convert_html_to_md(input);
        assert_eq!(result.unwrap(), expected);

        //
        // We want ATX style headers.
        let input: &str = r#"<p><h1>Title</h1></p>"#;
        let expected: &str = "# Title";

        let result = convert_html_to_md(input);
        assert_eq!(result.unwrap(), expected);
    }
}
