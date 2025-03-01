use crate::common::get_tag_attr;
use crate::dummy::IdentityHandler;
use percent_encoding::percent_decode_str;

use super::StructuredPrinter;
use super::TagHandler;

use markup5ever_rcdom::{Handle, NodeData};

#[derive(Default)]
pub struct AnchorHandler {
    start_pos: usize,
    url: String,
    emit_unchanged: bool,
}

impl TagHandler for AnchorHandler {
    fn handle(&mut self, tag: &Handle, printer: &mut StructuredPrinter) {
        // Check for a `name` attribute. If it exists, we can't support this
        // in markdown, so we must emit this tag unchanged.
        if get_tag_attr(tag, "name").is_some() {
            let mut identity = IdentityHandler::default();
            identity.handle(tag, printer);
            self.emit_unchanged = true;
        }

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
