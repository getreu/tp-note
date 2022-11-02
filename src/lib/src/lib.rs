//! The `tpnote-lib` library is designed to embed Tp-Note's core function
//! in common text editors and text editor plugins. It is dealing with templates
//! and input files and is also part of the command line application
//! [Tp-Note](https://blog.getreu.net/projects/tp-note/).  This library also
//! provides a default configuration in the static variable `LIB_CFG` that can
//! be customized at runtime. The defaults for the variables grouped in
//! `LIB_CFG`, are defined as constants in the module `config` (see Rustdoc).
//!
//! `tpnote-lib` offers a high-level API and a low-level API.
//! The high-level API, c.f. module `workflow` abstracts most implementation
//! details. Roughly speaking, the input path correspond to _Tp-Note_'s 
//! first positional command line parameter and the output path
//! is the same that is printed to stdout after usage.
//! The heart of the low-level API is the module `note`. Everything else
//! is designed around. In `note` you will find some Rustdoc tests illustrating
//! the usage of this API. Most other modules are not called directly and might
//! change to `private` in the future. The consumer of the low-level API in
//! the module `note` is the high-level API in the module `workflow`.
//!
//! The main consumer of `tpnote-lib`'s high-level API is the module 
//! `workflow` in `tp-note` crate.
//! In addition to the above-mentioned Rustdoc tests, the source code of
//! `tp-note::workflow` and `tpnote_lib::workflow` showcases the usage 
//! of the high-level API and of the low-level API the best.

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
