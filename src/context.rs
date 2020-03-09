//! Stores `tp-note`'s environment.
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
    ///     `sanitize_path()` below).
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
        let val_path = Self::sanitize_path(val);
        self.ct.insert(&key_path, &val_path);

        // We also register a `<key>-path-alpha` version, where <val> is
        // prepended with the `'` character, if <val> starts with number.
        // This guarantees, that <val> is alphabetic.
        let mut key_path_alpha = key.to_string();
        key_path_alpha.push_str("__alphapath");
        let mut val_path_alpha = val_path.to_string();
        let first_char = val.chars().next();
        if first_char.is_some() && (first_char.unwrap().is_numeric()) {
            val_path_alpha.insert(0, '\'');
        }
        self.ct.insert(&key_path_alpha, &val_path_alpha);
    }

    /// Filters filesystem critical characters:
    ///
    /// * Exclude NTFS critical characters:       `<>:"\\/|?*`
    /// [source](https://msdn.microsoft.com/en-us/library/windows/desktop/aa365247%28v=vs.85%29.aspx)
    /// * Exclude restricted in fat32:            `+,;=[]`
    /// [source](https://en.wikipedia.org/wiki/Filename#Reserved_characters_and_words)
    /// * These are considered unsafe in URLs:    `<>#%{}|\^~[]` `
    /// [source](https://perishablepress.com/stop-using-unsafe-characters-in-urls/)
    fn sanitize_path(s: &str) -> String {
        // proceed line by line
        s.lines()
            .map(|l| {
                let mut s = l
                    .chars()
                    // tab -> space
                    .map(|c| if c.is_whitespace() { ' ' } else { c })
                    // Delete control characters.
                    .filter(|c| !c.is_control())
                    // Replaces:  :\\/|?~,;=    ->    _
                    .map(|c| {
                        if c == ':'
                            || c == '\\'
                            || c == '/'
                            || c == '|'
                            || c == '?'
                            || c == '~'
                            || c == ','
                            || c == ';'
                            || c == '='
                        {
                            '_'
                        } else {
                            c
                        }
                    })
                    // Exclude NTFS critical characters:       <>:"\\/|?*
                    // https://msdn.microsoft.com/en-us/library/windows/desktop/aa365247%28v=vs.85%29.aspx
                    // Exclude restricted in fat32:            +,;=[]
                    // https://en.wikipedia.org/wiki/Filename#Reserved_characters_and_words
                    // These are considered unsafe in URLs:    <>#%{}|\^~[]`
                    // https://perishablepress.com/stop-using-unsafe-characters-in-urls/
                    .map(|c| {
                        if c == ':'
                            || c == '<'
                            || c == '>'
                            || c == ':'
                            || c == '"'
                            || c == '*'
                            || c == '#'
                            || c == '%'
                            || c == '{'
                            || c == '}'
                            || c == '^'
                            || c == '['
                            || c == ']'
                            || c == '+'
                            || c == '`'
                        {
                            ' '
                        } else {
                            c
                        }
                    })
                    .collect::<String>()
                    // trim beginning and end of line
                    .trim_matches(|c: char| c.is_whitespace() || c == '_' || c == '-')
                    .to_string();
                // Line sperarator
                s.push('-');
                s
            })
            .collect::<String>()
            // trim again beginning and end of the whole string
            .trim_matches(|c: char| c.is_whitespace() || c == '_' || c == '-')
            .to_string()
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
    use super::ContextWrapper;
    #[test]
    fn test_sanitize_path() {
        // test filter 1
        assert_eq!(
            ContextWrapper::sanitize_path("\tabc efg"),
            "abc efg".to_string()
        );
        // test filter 2
        assert_eq!(
            ContextWrapper::sanitize_path("abc\u{0019}efg"),
            "abcefg".to_string()
        );
        // test filter 3
        assert_eq!(
            ContextWrapper::sanitize_path("abc:\\/|?~,;=efg"),
            "abc_________efg".to_string()
        );
        // test filter4
        assert_eq!(
            ContextWrapper::sanitize_path("abc<>\"*<>#%{}^[]+[]`efg"),
            "abc                 efg".to_string()
        );
        // test replace Unix newline
        assert_eq!(
            ContextWrapper::sanitize_path("-_\ta?b?c \t >_-\n   efg"),
            "a_b_c-efg".to_string()
        );
        // test replace Window newline
        assert_eq!(
            ContextWrapper::sanitize_path("abc\r\nefg"),
            "abc-efg".to_string()
        );
    }
}
