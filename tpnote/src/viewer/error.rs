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
    /// Remedy: see `viewer.session_binding_cookie` in the configuration file.
    #[error("Connection rejected: missing or invalid viewer session cookie.")]
    SessionCookieMismatch,

    /// A bound viewer refused a request whose session cookie did not match
    /// (`viewer.session_binding_cookie`). `expected` is the token the viewer is
    /// bound to, `got` the cookie the client presented (`(missing)` if none).
    /// The values are logged for debugging only; the 403 page never shows the
    /// expected token (it may reach a hostile client).
    #[error(
        "Connection rejected: viewer session cookie mismatch \
         (expected: {expected}, got: {got})."
    )]
    SessionCookieRejected { expected: String, got: String },

    /// The connecting peer was proven to belong to a different OS user and was
    /// refused with `403 Forbidden` (`viewer.same_user_policy`). The viewer
    /// keeps running and keeps serving the legitimate user. `local_user` is the
    /// OS user running Tp-Note, `peer_user` the connecting (viewer) client.
    #[cfg(feature = "same-user-policy")]
    #[error(
        "Connection rejected: the client belongs to a different OS user \
         (local user: {local_user}, viewer user: {peer_user})."
    )]
    PeerUserMismatch {
        local_user: String,
        peer_user: String,
    },

    /// The connecting peer's OS user could not be determined and the policy is
    /// `Reject` (fail-closed), so the request was refused with `403 Forbidden`.
    /// `local_user` is the OS user running Tp-Note, `peer_user` the connecting
    /// (viewer) client (`unknown` here).
    /// Remedy: see `viewer.same_user_policy` in the configuration file.
    #[cfg(feature = "same-user-policy")]
    #[error(
        "Connection rejected: the client's OS user could not be determined \
         (local user: {local_user}, viewer user: {peer_user}; \
         `viewer.same_user_policy = Reject`)."
    )]
    PeerUserUnknown {
        local_user: String,
        peer_user: String,
    },

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

#[cfg(test)]
mod tests {
    use super::ViewerError;

    /// The peer-user rejection errors name both detected OS users.
    #[cfg(feature = "same-user-policy")]
    #[test]
    fn peer_user_errors_name_the_users() {
        let s = ViewerError::PeerUserMismatch {
            local_user: "getreu".to_string(),
            peer_user: "alice".to_string(),
        }
        .to_string();
        assert!(s.contains("local user: getreu"), "{s}");
        assert!(s.contains("viewer user: alice"), "{s}");

        let s = ViewerError::PeerUserUnknown {
            local_user: "getreu".to_string(),
            peer_user: "unknown".to_string(),
        }
        .to_string();
        assert!(s.contains("local user: getreu"), "{s}");
        assert!(s.contains("viewer user: unknown"), "{s}");
    }

    /// The cookie-mismatch error names the expected and presented cookies.
    #[test]
    fn session_cookie_error_names_expected_and_got() {
        let s = ViewerError::SessionCookieRejected {
            expected: "deadbeef".to_string(),
            got: "(missing)".to_string(),
        }
        .to_string();
        assert!(s.contains("expected: deadbeef"), "{s}");
        assert!(s.contains("got: (missing)"), "{s}");
    }
}
