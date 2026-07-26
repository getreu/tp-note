//! The viewer feature's error type.
use crate::error::ConfigFileError;
use core::str::Utf8Error;
use std::sync::mpsc::RecvError;
use thiserror::Error;
use tpnote_lib::error::{FileError, NoteError};

/// Represents an error in the viewer feature.
/// Hint: to see this error restart _Tp-Note_ with `--debug debug`.
#[derive(Debug, Error)]
pub enum ViewerError {
    /// In `update()` every HTTP client in `event_tx_list`
    /// receives a TCP message. If the client does not
    /// acknowledge this message, it is removed from the
    /// list. An empty list means that all clients have
    /// disconnected.
    #[error("All subscribers have disconnected.")]
    AllSubscriberDiconnected,

    /// Should not happen. Please report a bug.
    #[error("Can not view non-text files.")]
    MarkupLanguageNone,

    /// Should not happen. Please report a bug.
    #[error("URL path must start with `/`")]
    UrlMustStartWithSlash,

    /// Remedy: restart with `--debug trace` and make sure that
    /// no local process is attacking our HTTP server.
    /// If there are good reasons to allow more connections,
    /// raise the value `tcp_connections_max` in the
    /// configuration file.
    #[error(
        "Maximum open TCP connections ({max_conn}) exceeded. \
         Can not handle request. Consider raising the configuration variable \
         `tcp_connections_max` in the configuration file."
    )]
    TcpConnectionsExceeded { max_conn: usize },

    /// The request did not present the session cookie the viewer is bound
    /// to (or presented a foreign `Host` header). The request was refused
    /// with `403 Forbidden` and its connection is closed; the viewer itself
    /// keeps running and keeps serving the bound client.
    /// Remedy: see `viewer.session_binding` in the configuration file.
    #[error("Connection rejected: missing or invalid viewer session cookie.")]
    SessionCookieMismatch,

    /// The connecting peer was proven to belong to a different OS user and was
    /// refused with `403 Forbidden` (`viewer.same_user_policy`). The viewer
    /// keeps running and keeps serving the legitimate user.
    #[cfg(feature = "same-user-policy")]
    #[error("Connection rejected: the client belongs to a different OS user.")]
    PeerUserMismatch,

    /// The connecting peer's OS user could not be determined and the policy is
    /// `Reject` (fail-closed), so the request was refused with `403 Forbidden`.
    /// Remedy: see `viewer.same_user_policy` in the configuration file.
    #[cfg(feature = "same-user-policy")]
    #[error(
        "Connection rejected: the client's OS user could not be determined \
         (`viewer.same_user_policy = Reject`)."
    )]
    PeerUserUnknown,

    /// Network error.
    #[error("Can not read TCP stream: {error}")]
    StreamRead { error: std::io::Error },

    /// Network error.
    #[error("Can not parse HTTP header in TCP stream: {source_str}")]
    StreamParse { source_str: String },

    /// Remedy: Check the template syntax.
    #[error("Failed to render the HTML error page (cf. `{tmpl}` in configuration file).\n{source}")]
    RenderErrorPage { tmpl: String, source: NoteError },

    /// File access error.
    #[error(transparent)]
    File(#[from] FileError),

    /// Watcher error.
    #[error(transparent)]
    Notify(#[from] notify::Error),

    /// Network error.
    #[error(transparent)]
    Httparse(#[from] httparse::Error),

    /// Error in `sse_server::serve_event2()` when the watcher thread disconnects the `event`
    /// channel.
    #[error(transparent)]
    Recv(#[from] RecvError),

    /// Forward `FileError::ApplicationReturn` and `FileError::NoApplicationFound needed by
    /// `viewer::web_browser`.
    #[error(transparent)]
    ConfigFile(#[from] ConfigFileError),

    /// Forward errors from `error::NoteError` when rendering the page.
    #[error(transparent)]
    Note(#[from] NoteError),

    /// Error while decoding URL path.
    #[error(transparent)]
    Utf8(#[from] Utf8Error),

    /// Errors mostly related to the HTTP stream.
    #[error(transparent)]
    Io(#[from] std::io::Error),
}
