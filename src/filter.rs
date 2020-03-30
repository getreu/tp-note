extern crate sanitize_filename_reader_friendly;
use lazy_static::lazy_static;
use sanitize_filename_reader_friendly::sanitize;
use std::collections::HashMap;
use std::hash::BuildHasher;
use tera::{to_value, try_get_value, Result as TeraResult, Tera, Value};

lazy_static! {
    pub static ref TERA: Tera = {
        let mut tera = Tera::default();
        tera.register_filter("path", path_filter);
        tera
    };
}

pub fn path_filter<S: BuildHasher>(
    value: &Value,
    args: &HashMap<String, Value, S>,
) -> TeraResult<Value> {
    let p = try_get_value!("path", "value", String, value);

    let alpha_required = match args.get("alpha") {
        Some(val) => try_get_value!("path", "alpha", bool, val),
        None => false,
    };

    let mut filtered = sanitize(&p);

    if alpha_required {
        let first_char = filtered.chars().next();
        if let Some(c) = first_char {
            if c.is_numeric() {
                filtered.insert(0, '\'');
            }
        };
    };

    Ok(to_value(&filtered).unwrap())
}

#[cfg(test)]
mod tests {
    use super::path_filter;
    use std::collections::HashMap;
    use tera::to_value;

    #[test]
    fn test_path_filter() {
        let result = path_filter(
            &to_value(&".# Strange filename? Yes.").unwrap(),
            &HashMap::new(),
        );
        assert!(result.is_ok());
        assert_eq!(
            result.unwrap(),
            to_value(&"Strange filename_ Yes.").unwrap()
        );
    }

    #[test]
    fn test_path_filter_alpha() {
        let mut args = HashMap::new();
        args.insert("alpha".to_string(), to_value(true).unwrap());
        let result = path_filter(&to_value(&"1. My first: chapter").unwrap(), &args);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), to_value(&"'1. My first_ chapter").unwrap());
    }
}
