//! A library for the template tool _Tp-Note_ that abstracts notes, files and
//! configurable templates. Is is designed to be included in text file
//! editors. All default values are customizable at runtime.

pub mod config;
pub mod content;
pub mod context;
pub mod error;
pub mod filename;
pub mod filter;
pub mod front_matter;
pub mod note;
pub mod template;
