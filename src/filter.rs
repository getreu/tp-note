extern crate sanitize_filename_reader_friendly;
use lazy_static::lazy_static;
use sanitize_filename_reader_friendly::sanitize;
use std::collections::HashMap;
use std::hash::BuildHasher;
use std::ops::Deref;
use std::path::PathBuf;
use tera::{to_value, try_get_value, Result as TeraResult, Tera, Value};

lazy_static! {
/// Tera object with custom `path()` function registered.
    pub static ref TERA: Tera = {
        let mut tera = Tera::default();
        tera.register_filter("path", path_filter);
        tera
    };
}

/// Add a new filter to Tera templates:
/// `path` or `path()` sanitizes a string so that it can be used
/// to assemble filenames or paths.
/// In addition, `path(alpha=true)` prepends an apostroph when the result
/// starts with a number. This way we guaranty that the filename
/// never starts with a number. We do not allow this, to be able
/// to distinguish reliably the sort tag from the filename.
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

/// Tiny wrapper around Tera-context with some additional information.
#[derive(Debug, PartialEq)]
pub struct ContextWrapper {
    // Collection of substitution variables.
    ct: tera::Context,
    // The note's directory path on disk.
    pub fqpn: PathBuf,
}

/// A thin wrapper around `tera::Context` storing some additional
/// information.
impl ContextWrapper {
    pub fn new() -> Self {
        Self {
            ct: tera::Context::new(),
            fqpn: PathBuf::new(),
        }
    }

    /// Function that forwards a `kay-value` to the encapsulated `
    /// tera::Context::insert()` function.
    pub fn insert(&mut self, key: &str, val: &str) {
        // The first version is the unmodified variable `<key>` with original <val>.
        self.ct.insert(key, &val);
    }
}

/// Auto-dereference for convenient access to `tera::Content`.
impl Deref for ContextWrapper {
    type Target = tera::Context;

    fn deref(&self) -> &Self::Target {
        &self.ct
    }
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
