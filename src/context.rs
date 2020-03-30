//! Stores `tp-note`'s environment.
extern crate sanitize_filename_reader_friendly;
use sanitize_filename_reader_friendly::sanitize;
use std::ops::Deref;
use std::path::PathBuf;

#[derive(Debug, PartialEq)]
pub struct ContextWrapper {
    // Collection of substitution variables.
    ct: tera::Context,
    // The note's directory.
    pub fqpn: PathBuf,
}

/// A thin wrapper around `tera::Context` whose `insert()`-function
/// registers three variants of a given key-value pair (see below).
impl ContextWrapper {
    pub fn new() -> Self {
        Self {
            ct: tera::Context::new(),
            fqpn: PathBuf::new(),
        }
    }

    /// Function that forwards a `kay-value` to the encapsulated `
    /// tera::Context::insert()` function.
    /// In addition two variants of the original `key-value` pair
    /// are generated and also inserted:
    /// 1. `<key-name>__path`:
    ///     the string value is sanitized for filename usage (see
    ///     `sanitize_filename()` below).
    /// 2. `<key-name>__alphapath`:
    ///     If the sanitized string starts with a number digit
    ///     (`0`-`9`), the character `'` is prepended.
    pub fn insert(&mut self, key: &str, val: &str) {
        // The first version is the unmodified variable `<key>` with original <val>.
        self.ct.insert(key, &val);

        // We register the `<key>-path` with filename sanitized <val>.
        // This is for use in the filename template.
        let mut key_path = key.to_string();
        key_path.push_str("__path");
        let val_path = sanitize(val);
        self.ct.insert(&key_path, &val_path);

        // We also register a `<key>-path-alpha` version, where <val> is
        // prepended with the `'` character, if <val> starts with number.
        // This guarantees, that <val> is alphabetic.
        let mut key_path_alpha = key.to_string();
        key_path_alpha.push_str("__alphapath");
        let mut val_path_alpha = val_path.to_string();
        let first_char = val.chars().next();
        if let Some(c) = first_char {
            if c.is_numeric() {
                val_path_alpha.insert(0, '\'');
            }
        };
        self.ct.insert(&key_path_alpha, &val_path_alpha);
    }
}

/// Auto-dereference for convenient access to `tera::Content`.
impl Deref for ContextWrapper {
    type Target = tera::Context;

    fn deref(&self) -> &Self::Target {
        &self.ct
    }
}
