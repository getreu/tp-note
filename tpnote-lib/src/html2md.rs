//! This module abstracts the HTML to Markdown filter.
use std::collections::HashMap;

use crate::error::NoteError;
use html2md::{
    parse_html_custom, Handle, NodeData, StructuredPrinter, TagHandler, TagHandlerFactory,
};
use percent_encoding::percent_decode_str;

/*
// Alternative:
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
    struct CustomTagFactory;
    impl TagHandlerFactory for CustomTagFactory {
        fn instantiate(&self) -> Box<dyn TagHandler> {
            Box::new(CustomAnchorHandler::default())
        }
    }

    let mut tag_factory: HashMap<String, Box<dyn TagHandlerFactory>> = HashMap::new();
    tag_factory.insert(String::from("a"), Box::new(CustomTagFactory {}));

    Ok(parse_html_custom(html, &tag_factory))
}

#[derive(Default)]
pub struct CustomAnchorHandler {
    start_pos: usize,
    url: String,
    emit_unchanged: bool,
}

impl TagHandler for CustomAnchorHandler {
    fn handle(&mut self, tag: &Handle, printer: &mut StructuredPrinter) {
        // TODO include this when `IdentityHandler` becomes public.
        // // Check for a `name` attribute. If it exists, we can't support this
        // // in markdown, so we must emit this tag unchanged.
        // if let Some(get_tag_attr) = get_tag_attr(tag, "name") {
        //     let mut identity = IdentityHandler::default();
        //     identity.handle(tag, printer);
        //     self.emit_unchanged = true;
        // }

        self.start_pos = printer.data.len();

        // try to extract a hyperlink
        self.url = match tag.data {
            NodeData::Element { ref attrs, .. } => {
                let attrs = attrs.borrow();
                let href = attrs
                    .iter()
                    .find(|attr| attr.name.local.to_string() == "href");
                match href {
                    Some(link) => {
                        let link = &*link.value;
                        let link = percent_decode_str(link).decode_utf8().unwrap_or_default();

                        if link.contains(|c: char| c.is_ascii_whitespace()) {
                            format!("<{}>", link)
                        } else {
                            link.to_string()
                        }
                    }
                    None => String::new(),
                }
            }
            _ => String::new(),
        };
    }

    fn after_handle(&mut self, printer: &mut StructuredPrinter) {
        if !self.emit_unchanged {
            // add braces around already present text, put an url afterwards
            printer.insert_str(self.start_pos, "[");
            printer.append_str(&format!("]({})", self.url))
        }
    }
}
