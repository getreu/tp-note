use super::StructuredPrinter;
use super::TagHandler;
use crate::markup5ever_rcdom::{Handle, NodeData};

#[derive(Default)]
pub struct CodeHandler {
    code_type: String,
}

impl CodeHandler {
    fn find_code_child(handle: &Handle) -> Option<Handle> {
        for child in handle.children.borrow().iter() {
            if let NodeData::Element { ref name, .. } = child.data
                && name.local.as_ref() == "code" {
                    return Some(child.clone());
                }
        }
        None
    }

    /// Used in both starting and finishing handling
    fn do_handle(&mut self, printer: &mut StructuredPrinter, start: Option<&Handle>) {
        let immediate_parent = printer.parent_chain.last().unwrap().to_owned();
        if self.code_type == "code" && immediate_parent == "pre" {
            // we are already in "code" mode
            return;
        }

        match self.code_type.as_ref() {
            "pre" => {
                // code block should have its own paragraph
                if start.is_some() {
                    printer.insert_newline();
                }
                printer.append_str("\n```");

                if let Some(handle) = start.and_then(Self::find_code_child)
                    && let NodeData::Element { ref attrs, .. } = handle.data {
                        let attrs = attrs.borrow();
                        let class = attrs
                            .iter()
                            .find(|attr| attr.name.local.to_string() == "class");
                        if let Some(class) = class {
                            let class = &*class.value;
                            let lang = class
                                .split(" ")
                                .filter_map(|v| v.strip_prefix("language-"))
                                .next();
                            if let Some(lang) = lang {
                                printer.append_str(lang);
                            }
                        }
                    }

                printer.insert_newline();

                if start.is_none() {
                    printer.insert_newline();
                }
            }
            "code" | "samp" => printer.append_str("`"),
            _ => {}
        }
    }
}

impl TagHandler for CodeHandler {
    fn handle(&mut self, tag: &Handle, printer: &mut StructuredPrinter) {
        self.code_type = match tag.data {
            NodeData::Element { ref name, .. } => name.local.to_string(),
            _ => String::new(),
        };

        self.do_handle(printer, Some(tag));
    }
    fn after_handle(&mut self, printer: &mut StructuredPrinter) {
        self.do_handle(printer, None);
    }
}
