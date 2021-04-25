//! The viewer feature's error type.
extern crate httparse;
extern crate notify;

use crate::error::FileError;
use crate::process_ext::ChildExtError;
use core::str::Utf8Error;
use std::sync::mpsc::RecvError;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ViewerError {
    /// The watched file was moved by another process.
    #[error("Watcher: lost observed file. File was renamed.")]
    LostRenamedFile,

    /// Should not happen. Please report a bug.
    #[error("Can not view non-text files.")]
    MarkupLanguageNone,

    /// Should not happen. Please report a bug.
    #[error("URL path must start with `/`")]
    UrlMustStartWithSlash,

    /// Network error.
    #[error("Can not read from TCP stream.")]
    StreamRead { source_str: String },

    /// Remedy: check `browser_args` configuration file variable.
    #[error("Error executing external application.")]
    ChildExt {
        #[from]
        source: ChildExtError,
    },

    /// Watcher error.
    #[error(transparent)]
    Notify(#[from] notify::Error),

    /// Network error.
    #[error(transparent)]
    Httparse(#[from] httparse::Error),

    /// Error in `sse_server::render_content_and_errror()` mainly while rendering the error page.
    #[error(transparent)]
    Rendition(#[from] Box<dyn std::error::Error>),

    /// Error in `sse_server::serve_event2()` when the watcher thread disconnects the `event`
    /// channel.
    #[error(transparent)]
    Recv(#[from] RecvError),

    /// `viewer::web_browser` needs `FileError::ApplicationReturn` and
    /// `FileError::NoApplicationFound`.
    #[error(transparent)]
    File(#[from] FileError),

    /// Error in `sse_server::ercent_decode_str().decode_utf8()`
    #[error(transparent)]
    Utf8(#[from] Utf8Error),

    /// Errors mostly related to the HTTP stream.
    #[error(transparent)]
    Io(#[from] std::io::Error),
}