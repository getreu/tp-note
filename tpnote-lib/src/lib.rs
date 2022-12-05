//! The `tpnote-lib` library is designed to embed Tp-Note's core function in
//! common text editors and text editor plugins. It is dealing with templates
//! and input files and is also part of the command line application
//! [Tp-Note](https://blog.getreu.net/projects/tp-note/).  This library also
//! provides a default configuration in the static variable `LIB_CFG` that can
//! be customized at runtime. The defaults for the variables grouped in
//! `LIB_CFG`, are defined as constants in the module `config` (see Rustdoc).
//!
//! Tp-Note's high-level API, c.f. module `workflow`, abstracts most
//! implementation details. Roughly speaking, the input path correspond to
//! _Tp-Note_'s first positional command line parameter and the output path is
//! the same that is printed to stdout after usage. The main consumer of
//! `tpnote-lib`'s high-level API is the module `workflow` in `tp-note` crate.
//!
pub mod config;
pub mod content;
pub mod context;
pub mod error;
pub mod filename;
mod filter;
mod front_matter;
pub mod html;
pub mod markup_language;
mod note;
pub mod template;
pub mod workflow;
