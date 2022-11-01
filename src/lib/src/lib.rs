//! The `tpnote-lib` library is designed to embed Tp-Note's core function
//! in common text editors and text editor plugins. It is dealing with templates
//! and input files and is also part of the command line application
//! [Tp-Note](https://blog.getreu.net/projects/tp-note/).  This library also
//! provides a default configuration in the static variable `LIB_CFG` that can
//! be customized at runtime. The defaults for the variables grouped in
//! `LIB_CFG`, are defined as constants in the module `config` (see Rustdoc).
//!
//! This heart of this API is the module `note`. Everything else
//! is designed around. In `note` you will find some Rustdoc tests illustrating
//! the usage of this API. Most other modules :ware not called directly and might
//! change to `private` in the future.
//!
//! The main consumer of this API is the module `workflow` in the crate `tpnote`.
//! In addition to the above-mentioned Rustdoc tests, the source code of
//! `workflow` showcases its usage the best.

pub mod config;
pub mod content;
pub mod context;
pub mod error;
pub mod filename;
pub mod filter;
pub mod front_matter;
pub mod note;
pub mod template;
pub mod workflow;
