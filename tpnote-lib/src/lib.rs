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
//! the same that is printed to stdout after usage. The main consumer of
//! `tpnote-lib`'s high-level API is the module `workflow` and `html_renderer`
//! in the `tpnote` crate.
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
pub mod markup_language;
mod note;
pub mod settings;
pub mod template;
pub mod workflow;
