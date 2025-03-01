use super::StructuredPrinter;
use super::TagHandler;

use markup5ever_rcdom::{Handle, NodeData};

#[derive(Default)]
pub struct StyleHandler {
    start_pos: usize,
    style_type: String,
}

/// Applies givem `mark` at both start and end indices, updates printer position to the end of text
fn apply_at_bounds(printer: &mut StructuredPrinter, start: usize, end: usize, mark: &str) {
    printer.data.insert_str(end, mark);
    printer.data.insert_str(start, mark);
}

impl TagHandler for StyleHandler {
    fn handle(&mut self, tag: &Handle, printer: &mut StructuredPrinter) {
        self.start_pos = printer.data.len();
        self.style_type = match tag.data {
            NodeData::Element { ref name, .. } => name.local.to_string(),
            _ => String::new(),
        };
    }

    fn after_handle(&mut self, printer: &mut StructuredPrinter) {
        let non_space_offset = printer.data[self.start_pos..].find(|ch: char| !ch.is_whitespace());
        if non_space_offset.is_none() {
            // only spaces or no text at all
            return;
        }

        let first_non_space_pos = self.start_pos + non_space_offset.unwrap();
        let last_non_space_pos = printer
            .data
            .trim_end_matches(|ch: char| ch.is_whitespace())
            .len();

        // finishing markup
        match self.style_type.as_ref() {
            "b" | "strong" => {
                apply_at_bounds(printer, first_non_space_pos, last_non_space_pos, "**")
            }
            "i" | "em" => apply_at_bounds(printer, first_non_space_pos, last_non_space_pos, "*"),
            "s" | "del" => apply_at_bounds(printer, first_non_space_pos, last_non_space_pos, "~~"),
            "u" | "ins" => apply_at_bounds(printer, first_non_space_pos, last_non_space_pos, "__"),
            _ => {}
        }
    }
}
