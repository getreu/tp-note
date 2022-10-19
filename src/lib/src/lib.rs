//! The `tpnote-lib` library is designed to embed Tp-Note's core function
//! in common text editors and text editor plugins. It is dealing with templates
//! and input files and is also part of the command line application
//! [Tp-Note](https://blog.getreu.net/projects/tp-note/).  The library
//! provides a default configuration in the static variable `LIB_CFG` that can
//! be customized at runtime.

pub mod config;
pub mod content;
pub mod context;
pub mod error;
pub mod filename;
pub mod filter;
pub mod front_matter;
pub mod note;
pub mod template;
