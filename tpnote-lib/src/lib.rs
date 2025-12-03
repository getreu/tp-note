//! The `tpnote-lib` library is designed to embed Tp-Note's core function in
//! common text editors and text editor plugins. It is dealing with templates
//! and input files and is also part of the command line application
//! [Tp-Note](https://blog.getreu.net/projects/tp-note/). This library also
//! provides a default configuration in the static variable `LIB_CFG` that can
//! be customized at runtime. The defaults for the variables grouped in
//! `LIB_CFG`, are defined as constants in the module `config` (see Rustdoc).
//! While `LIB_CFG` is sourced only once at the start of Tp-Note, the
//! `SETTINGS` may be sourced more often. The latter contains configuration
//! data originating form environment variables.
//!
//! Tp-Note's high-level API, cf. module `workflow`, abstracts most
//! implementation details. Roughly speaking, the input path correspond to
//! _Tp-Note's_ first positional command line parameter and the output path is
//! the same that is printed to standard output after usage. The main
//! consumer of `tpnote-lib`'s high-level API is the module `workflow` and
//! `html_renderer` in the `tpnote` crate.
//!
pub mod clone_ext;
pub mod config;
pub mod config_value;
pub mod content;
pub mod context;
pub mod error;
pub mod filename;
mod filter;
mod front_matter;
#[cfg(feature = "renderer")]
pub mod highlight;
pub mod html;
#[cfg(feature = "renderer")]
pub mod html2md;
pub mod html_renderer;
#[cfg(feature = "lang-detection")]
pub mod lingua;
pub mod markup_language;
mod note;
pub mod settings;
pub mod template;
pub mod text_reader;
pub mod workflow;

use std::iter::FusedIterator;

/// An iterator adapter that flattens an iterator of iterators,
/// while providing the index of the current outer (inner-producing) element.
pub struct FlattenWithIndex<I>
where
    I: Iterator,
    I::Item: IntoIterator,
{
    iter: I,
    current_inner: Option<<I::Item as IntoIterator>::IntoIter>,
    outer_index: usize, // This is the counter you asked for
}

impl<I> FlattenWithIndex<I>
where
    I: Iterator,
    I::Item: IntoIterator,
{
    /// Creates a new `FlattenWithIndex`.
    pub fn new(iter: I) -> Self {
        Self {
            iter,
            current_inner: None,
            outer_index: 0,
        }
    }
}

impl<I> Iterator for FlattenWithIndex<I>
where
    I: Iterator,
    I::Item: IntoIterator,
{
    type Item = (usize, <I::Item as IntoIterator>::Item);

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            // If we have a current inner iterator, try to get the next element from it
            if let Some(inner) = &mut self.current_inner {
                if let Some(item) = inner.next() {
                    return Some((self.outer_index - 1, item)); // -1 because we already incremented
                }
            }

            // Current inner is exhausted (or None), get the next outer element
            let next_outer = self.iter.next()?;
            self.current_inner = Some(next_outer.into_iter());
            self.outer_index += 1;
            // Loop back to try the new inner iterator
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let (inner_lower, inner_upper) = self
            .current_inner
            .as_ref()
            .map_or((0, None), |inner| inner.size_hint());

        let (outer_lower, outer_upper) = self.iter.size_hint();

        let lower = inner_lower.saturating_add(outer_lower);
        let upper = match (inner_upper, outer_upper) {
            (Some(i), Some(o)) => i.checked_add(o),
            _ => None,
        };

        (lower, upper)
    }
}

// Optional: implement FusedIterator if the underlying iterators do
impl<I> FusedIterator for FlattenWithIndex<I>
where
    I: Iterator + FusedIterator,
    I::Item: IntoIterator,
    <I::Item as IntoIterator>::IntoIter: FusedIterator,
{
}

pub trait FlattenWithIndexExt: Iterator {
    fn flatten_with_index(self) -> FlattenWithIndex<Self>
    where
        Self::Item: IntoIterator,
        Self: Sized,
    {
        FlattenWithIndex::new(self)
    }
}

impl<T: Iterator> FlattenWithIndexExt for T {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_flatten_with_index() {
        // Test with a non-empty outer iterator with multiple non-empty inner iterators
        let data = vec![vec!['a', 'b'], vec!['c', 'd'], vec!['e', 'f', 'g']];

        let result: Vec<(usize, char)> = data.into_iter().flatten_with_index().collect();

        let expected = vec![
            (0, 'a'),
            (0, 'b'),
            (1, 'c'),
            (1, 'd'),
            (2, 'e'),
            (2, 'f'),
            (2, 'g'),
        ];
        assert_eq!(result, expected);

        // Test with an empty outer iterator
        let data: Vec<Vec<char>> = Vec::new();
        let result: Vec<(usize, char)> = data.into_iter().flatten_with_index().collect();
        assert!(result.is_empty());

        // Test with an empty inner iterator (outer iterator is not empty)
        let data = vec![
            vec!['a', 'b'],
            vec![], // Empty inner iterator
            vec!['c', 'd'],
        ];

        let result: Vec<(usize, char)> = data.into_iter().flatten_with_index().collect();

        let expected = vec![(0, 'a'), (0, 'b'), (2, 'c'), (2, 'd')];
        assert_eq!(result, expected);

        // Test with a mix of non-empty and empty inner iterators
        let data = vec![
            vec!['a', 'b'],
            vec![], // Empty inner
            vec!['c'],
            vec![], // Empty inner
            vec!['d', 'e', 'f'],
        ];

        let result: Vec<(usize, char)> = data.into_iter().flatten_with_index().collect();

        let expected = vec![(0, 'a'), (0, 'b'), (2, 'c'), (4, 'd'), (4, 'e'), (4, 'f')];

        assert_eq!(result, expected);

        // Test with all empty inner iterators
        let data = vec![vec![], vec![], vec![]];

        let result: Vec<(usize, char)> = data.into_iter().flatten_with_index().collect();

        assert!(result.is_empty());

        // Test with just one element in the outer iterator
        let data = vec![vec!['a', 'b', 'c']];

        let result: Vec<(usize, char)> = data.into_iter().flatten_with_index().collect();

        let expected = vec![(0, 'a'), (0, 'b'), (0, 'c')];

        assert_eq!(result, expected);

        // Test with just one element in the inner iterator (outer iterator has multiple elements)
        let data = vec![
            vec!['a'], // Inner iterator has one element
            vec!['b', 'c'], // Inner iterator has one element
            vec!['d'], // Inner iterator has one element
        ];

        let result: Vec<(usize, char)> = data.into_iter().flatten_with_index().collect();

        let expected = vec![(0, 'a'), (1, 'b'), (1, 'c'), (2, 'd')];

        assert_eq!(result, expected);
    }
}
