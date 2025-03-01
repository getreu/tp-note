use super::TagHandler;
use super::StructuredPrinter;

use crate::common::get_tag_attr;
use crate::dummy::IdentityHandler;

use markup5ever_rcdom::Handle;

use percent_encoding::{utf8_percent_encode, AsciiSet, CONTROLS};

const FRAGMENT: &AsciiSet = &CONTROLS.add(b' ').add(b'"').add(b'<').add(b'>').add(b'`');

/// Handler for `<img>` tag. Depending on circumstances can produce both
/// inline HTML-formatted image and Markdown native one
#[derive(Default)]
pub struct ImgHandler {
    block_mode: bool
}

impl TagHandler for ImgHandler {

    fn handle(&mut self, tag: &Handle, printer: &mut StructuredPrinter) {
        // hack: detect if the image has associated style and has display in block mode
        let style_tag = get_tag_attr(tag, "src");
        if let Some(style) = style_tag {
            if style.contains("display: block") {
                self.block_mode = true
            }
        }

        if self.block_mode {
            // make image on new paragraph
            printer.insert_newline();
            printer.insert_newline();
        }

        // try to extract attrs
        let src = get_tag_attr(tag, "src");
        let alt = get_tag_attr(tag, "alt");
        let title = get_tag_attr(tag, "title");
        let height = get_tag_attr(tag, "height");
        let width = get_tag_attr(tag, "width");
        let align = get_tag_attr(tag, "align");

        if height.is_some() || width.is_some() || align.is_some() {
            // need to handle it as inline html to preserve attributes we support
            let mut identity = IdentityHandler::default();
            identity.handle(tag, printer);
        } else {
            // need to escape URL if it contains spaces
            // don't have any geometry-controlling attrs, post markdown natively
            let mut img_url = src.unwrap_or_default();
            if img_url.contains(' ') {
                img_url = utf8_percent_encode(&img_url, FRAGMENT).to_string();
            }

            printer.append_str(
                &format!("![{}]({}{})", 
                    alt.unwrap_or_default(), 
                    &img_url,
                    title.map(|value| format!(" \"{}\"", value)).unwrap_or_default()));
        }
    }

    fn after_handle(&mut self, printer: &mut StructuredPrinter) {
        if self.block_mode {
            printer.insert_newline();
            printer.insert_newline();
        }
    }
}