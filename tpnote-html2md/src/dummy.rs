use super::TagHandler;
use super::StructuredPrinter;

use html5ever::serialize;
use html5ever::serialize::{SerializeOpts, TraversalScope};
use markup5ever_rcdom::{Handle, NodeData, SerializableHandle};

#[derive(Default)]
pub struct DummyHandler;

impl TagHandler for DummyHandler {

    fn handle(&mut self, _tag: &Handle, _printer: &mut StructuredPrinter) {

    }

    fn after_handle(&mut self, _printer: &mut StructuredPrinter) {

    }
}

/// Handler that completely copies tag to printer as HTML with all descendants
#[derive(Default)]
pub(super) struct IdentityHandler;

impl TagHandler for IdentityHandler {

    fn handle(&mut self, tag: &Handle, printer: &mut StructuredPrinter) {
        let mut buffer = vec![];

        let options = SerializeOpts { traversal_scope: TraversalScope::IncludeNode, .. Default::default() };
        let to_be_serialized = SerializableHandle::from(tag.clone());
        let result = serialize(&mut buffer, &to_be_serialized, options);
        if result.is_err() {
            // couldn't serialize the tag
            return;
        }

        let conv = String::from_utf8(buffer);
        if conv.is_err() {
            // is non-utf8 string possible in html5ever?
            return;
        }

        printer.append_str(&conv.unwrap());
    }

    fn skip_descendants(&self) -> bool {
        return true;
    }

    fn after_handle(&mut self, _printer: &mut StructuredPrinter) {

    }
}

/// Handler that copies just one tag and doesn't skip descendants
#[derive(Default)]
pub struct HtmlCherryPickHandler {
    tag_name: String
}

impl TagHandler for HtmlCherryPickHandler {

    fn handle(&mut self, tag: &Handle, printer: &mut StructuredPrinter) {
        match tag.data {
            NodeData::Element { ref name, ref attrs, .. } => {
                let attrs = attrs.borrow();
                self.tag_name = name.local.to_string();

                printer.append_str(&format!("<{}", self.tag_name));
                for attr in attrs.iter() {
                    printer.append_str(&format!(" {}=\"{}\"", attr.name.local, attr.value));
                }
                printer.append_str(">");
            }
            _ => return
        }
    }

    fn skip_descendants(&self) -> bool {
        return false;
    }

    fn after_handle(&mut self, printer: &mut StructuredPrinter) {
        printer.append_str(&format!("</{}>", self.tag_name));
    }
}