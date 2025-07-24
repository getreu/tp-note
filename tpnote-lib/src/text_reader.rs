//! An iterator adapter to suppress CRLF (`\r\n`) sequences in a stream of
//! bytes.
//!
//! # Overview
//!
//! This module provides [`CrlfSuppressor`], an iterator adapter to filter out
//! CR (`\r`, 0x0D) when it is immediately followed by LF (`\n`, 0x0A), as
//! commonly found in Windows line endings.
//!
//! It also provides an extension trait [`CrlfSuppressorExt`] so you can easily
//! call `.crlf_suppressor()` on any iterator over bytes (e.g., from
//! `BufReader::bytes()`).
//!
//! # Usage
//!
//! ## Basic example
//!
//! ```rust
//! use std::io::{Cursor, Error, Read};
//! use tpnote_lib::text_reader::CrlfSuppressorExt;
//!
//! let data = b"hello\r\nworld";
//! let normalized: Result<Vec<u8>, Error> = Cursor::new(data)
//!     .bytes()
//!     .crlf_suppressor()
//!     .collect();
//! let s = String::from_utf8(normalized.unwrap()).unwrap();
//! assert_eq!(s, "hello\nworld");
//! ```
//!
//! ## Reading from a file
//!
//! ```rust,no_run
//! use std::fs::File;
//! use tpnote_lib::text_reader::read_as_string_with_crlf_suppression;
//!
//! let normalized = read_as_string_with_crlf_suppression(File::open("file.txt")?)?;
//! println!("{}", normalized);
//! # Ok::<(), std::io::Error>(())
//! ```
//!
//! # Implementation details
//!
//! In UTF-8, continuation bytes for multi-byte code points are always in the
//! range `0x80..0xBF`. Since `0x0D` and `0x0A` are not in this range, searching
//! for CRLF as byte values is safe.
//!
//! # See also
//!
//! - [`BufReader::bytes`](https://doc.rust-lang.org/std/io/struct.BufReader.html#method.bytes)
//! - [`String::from_utf8`](https://doc.rust-lang.org/std/string/struct.String.html#method.from_utf8)

use std::io::{self, BufReader, Read};
use std::iter::Peekable;

const CR: u8 = 0x0D; // Carriage Return.
const LF: u8 = 0x0A; // Line Feed.

/// An iterator adapter that suppresses CR (`\r`, 0x0D) when followed by LF
/// (`\n`, 0x0A). In a valid multi-byte UTF-8 sequence, continuation bytes must
/// be in the range 0x80 to 0xBF. As 0x0D and 0x0A are not in this range, we can
/// search for them in a stream of bytes.
///
/// * In UTF-8, multi-byte code points (3 or more bytes) have specific "marker"
///   bits in each byte:
/// * The first byte starts with 1110xxxx (for 3 bytes) or 11110xxx (for 4
///   bytes). Continuation bytes always start with 10xxxxxx (0x80..0xBF).
/// * 0x0D is 00001101 and 0x0A is 00001010—neither match the required bit
///   patterns for multi-byte UTF-8 encoding.
/// * In a valid multi-byte UTF-8 sequence, continuation bytes must be in the
///   range 0x80 to 0xBF.
/// * 0x0D and 0x0A are not in this range.
///
pub struct CrlfSuppressor<I: Iterator<Item = io::Result<u8>>> {
    iter: Peekable<I>,
}

impl<I: Iterator<Item = io::Result<u8>>> CrlfSuppressor<I> {
    /// Creates a new suppressor from an iterator over bytes.
    /// (Preferred usage: see extension trait `CrlfSuppressorExt`).
    ///
    /// # Example
    /// ```
    /// use std::io::Cursor;
    /// use std::io::Read;
    /// use tpnote_lib::text_reader::CrlfSuppressor;
    ///
    /// let bytes = b"foo\r\nbar";
    /// let suppressor = CrlfSuppressor::new(Cursor::new(bytes).bytes());
    /// ```
    /// Create a new suppressor from an iterator over bytes.
    pub fn new(iter: I) -> Self {
        Self {
            iter: iter.peekable(),
        }
    }
}

impl<I: Iterator<Item = io::Result<u8>>> Iterator for CrlfSuppressor<I> {
    type Item = io::Result<u8>;

    fn next(&mut self) -> Option<Self::Item> {
        match self.iter.next()? {
            Ok(CR) => match self.iter.peek() {
                Some(Ok(LF)) => {
                    self.iter.next(); // Consume.
                    Some(Ok(LF))
                }
                _ => Some(Ok(CR)),
            },
            Ok(byte) => Some(Ok(byte)),
            Err(err) => Some(Err(err)),
        }
    }
}
/// Extension trait to add `.crlf_suppressor()` to any iterator over bytes.
///
/// # Example
/// ```rust
/// use std::io::{Cursor, Error, Read};
/// use tpnote_lib::text_reader::CrlfSuppressorExt;
///
/// let data = b"hello\r\nworld";
/// let normalized: Result<Vec<u8>, Error> = Cursor::new(data)
///     .bytes()
///     .crlf_suppressor()
///     .collect();
/// let s = String::from_utf8(normalized.unwrap()).unwrap();
/// assert_eq!(s, "hello\nworld");
/// ```
pub trait CrlfSuppressorExt: Iterator<Item = io::Result<u8>> + Sized {
    /// Returns an iterator that suppresses CRLF sequences.
    fn crlf_suppressor(self) -> CrlfSuppressor<Self> {
        CrlfSuppressor::new(self)
    }
}

impl<T: Iterator<Item = io::Result<u8>>> CrlfSuppressorExt for T {}

/// Reads all bytes from the given reader, suppressing CR (`\r`) bytes that are
/// immediately followed by LF (`\n`).
///
/// This function is intended to normalize line endings by removing carriage
/// return characters that precede line feeds (i.e., converting CRLF sequences
/// to LF).
///
/// # Arguments
///
/// * `reader` - Any type that implements [`std::io::Read`], such as a file,
///   buffer, or stream.
///
/// # Returns
///
/// A [`std::io::Result`] containing a `Vec<u8>` with the filtered bytes, or an
/// error if one occurs while reading from the input.
///
/// # Example
///
/// ```rust
/// use std::io::Cursor;
/// use tpnote_lib::text_reader::read_with_crlf_suppression;
///
/// let data = b"foo\r\nbar\nbaz\r\n";
/// let cursor = Cursor::new(data);
/// let result = read_with_crlf_suppression(cursor).unwrap();
/// assert_eq!(result, b"foo\nbar\nbaz\n");
/// ```
///
/// # Errors
///
/// Returns any I/O error encountered while reading from the provided reader.
///
/// # See Also
///
/// [`std::io::Read`], [`std::fs::File`]
pub fn read_with_crlf_suppression<R: Read>(reader: R) -> io::Result<Vec<u8>> {
    let reader = BufReader::new(reader);
    let filtered_bytes = reader.bytes().crlf_suppressor();
    filtered_bytes.collect()
}

/// Reads all bytes from the given reader, suppressing CR (`\r`) bytes that are
/// immediately followed by LF (`\n`), and returns the resulting data as a UTF-8
/// string.
///
/// This function is useful for normalizing line endings (converting CRLF to LF)
/// and reading textual data from any source that implements [`std::io::Read`].
///
/// # Arguments
///
/// * `reader` - Any type implementing [`std::io::Read`], such as a file,
///   buffer, or stream.
///
/// # Returns
///
/// Returns an [`std::io::Result`] containing the resulting `String` if all
/// bytes are valid UTF-8, or an error if reading fails or the data is not valid
/// UTF-8.
///
/// # Errors
///
/// Returns an error if an I/O error occurs while reading, or if the data read
/// is not valid UTF-8.
///
/// # Example
///
/// ```rust
/// use std::io::Cursor;
/// use tpnote_lib::text_reader::read_as_string_with_crlf_suppression;
///
/// let input = b"hello\r\nworld";
/// let cursor = Cursor::new(input);
/// let output = read_as_string_with_crlf_suppression(cursor).unwrap();
/// assert_eq!(output, "hello\nworld");
/// ```
///
/// # See Also
///
/// [`read_with_crlf_suppression`]
pub fn read_as_string_with_crlf_suppression<R: Read>(reader: R) -> io::Result<String> {
    let bytes = read_with_crlf_suppression(reader)?;
    String::from_utf8(bytes).map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))
}

/// Additional method for `String` suppressing `\r` in `\r\n` sequences:
/// When no `\r\n` is found, no memory allocation occurs.
///
/// ```rust
/// use tpnote_lib::text_reader::StringExt;
///
/// let s = "hello\r\nworld".to_string();
/// let res = s.crlf_suppressor_string();
/// assert_eq!("hello\nworld", res);
///
/// let s = "hello\nworld".to_string();
/// let res = s.crlf_suppressor_string();
/// assert_eq!("hello\nworld", res);
/// ```
pub trait StringExt {
    fn crlf_suppressor_string(self) -> String;
}

impl StringExt for String {
    fn crlf_suppressor_string(self) -> String {
        // Replace `\r\n` with `\n`.
        // Searching in bytes is faster than in chars.
        // In UTF-8, continuation bytes for multi-byte code points are always in the
        // range `0x80..0xBF`. Since `0x0D` and `0x0A` are not in this range, searching
        // for CRLF as byte values is safe.
        if !self.contains("\r\n") {
            // Forward without allocating.
            self
        } else {
            // We allocate here and do a lot of copying.
            self.replace("\r\n", "\n")
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    fn run(input: &[u8]) -> String {
        let cursor = Cursor::new(input);
        let bytes = cursor.bytes().crlf_suppressor();
        let vec: Vec<u8> = bytes.map(|b| b.unwrap()).collect();
        String::from_utf8(vec).unwrap()
    }

    #[test]
    fn test_crlf_sequence() {
        let input = b"foo\r\nbar\r\nbaz";
        let expected = "foo\nbar\nbaz";
        assert_eq!(run(input), expected);
    }

    #[test]
    fn test_lone_cr() {
        let input = b"foo\rbar";
        let expected = "foo\rbar";
        assert_eq!(run(input), expected);
    }

    #[test]
    fn test_lone_lf() {
        let input = b"foo\nbar";
        let expected = "foo\nbar";
        assert_eq!(run(input), expected);
    }

    #[test]
    fn test_mixed_endings() {
        let input = b"foo\r\nbar\rbaz\nqux";
        let expected = "foo\nbar\rbaz\nqux";
        assert_eq!(run(input), expected);
    }

    #[test]
    fn test_empty_input() {
        let input = b"";
        let expected = "";
        assert_eq!(run(input), expected);
    }

    #[test]
    fn test_only_crlf() {
        let input = b"\r\n";
        let expected = "\n";
        assert_eq!(run(input), expected);
    }

    #[test]
    fn test_only_cr() {
        let input = b"\r";
        let expected = "\r";
        assert_eq!(run(input), expected);
    }

    #[test]
    fn test_only_lf() {
        let input = b"\n";
        let expected = "\n";
        assert_eq!(run(input), expected);
    }

    #[test]
    fn test_trailing_cr() {
        let input = b"foo\r";
        let expected = "foo\r";
        assert_eq!(run(input), expected);
    }

    #[test]
    fn test_trailing_crlf() {
        let input = b"foo\r\n";
        let expected = "foo\n";
        assert_eq!(run(input), expected);
    }

    #[test]
    fn test_crlf_suppressor_string() {
        use std::ptr::addr_of;
        let s = "hello\r\nworld".to_string();
        let s_addr = addr_of!(*s);
        let res = s.crlf_suppressor_string();
        assert_eq!("hello\nworld", res);
        // Memory allocation occurred.
        assert_ne!(s_addr, addr_of!(*res));

        //
        let s = "hello\nworld".to_string();
        let s_addr = addr_of!(*s);
        let res = s.crlf_suppressor_string();
        assert_eq!("hello\nworld", res);
        // No memory allocation here:
        assert_eq!(s_addr, addr_of!(*res));
    }
}
