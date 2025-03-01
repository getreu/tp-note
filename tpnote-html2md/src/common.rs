use markup5ever_rcdom::{Handle, NodeData};

pub fn get_tag_attr(tag: &Handle, attr_name: &str) -> Option<String> {
    match tag.data {
        NodeData::Element { ref attrs, .. } => {
            let attrs = attrs.borrow();
            let requested_attr = attrs
                .iter()
                .find(|attr| attr.name.local.to_string() == attr_name);
            requested_attr.map(|attr| attr.value.to_string())
        }
        _ => None,
    }
}
