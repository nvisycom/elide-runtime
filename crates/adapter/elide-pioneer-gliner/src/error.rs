//! Error translation: Pioneer failures → [`elide_core::Error`].
//!
//! Crate-private — the public API reports [`elide_core::Error`]; this is
//! the internal seam the client uses before bubbling up.

use elide_core::{Error, ErrorKind};

/// Errors surfaced internally by the Pioneer backend.
#[derive(Debug, thiserror::Error)]
pub(crate) enum PioneerError {
    /// HTTP / transport failure — client construction, network I/O.
    #[error("pioneer transport error: {0}")]
    Transport(#[from] reqwest::Error),
    /// The API rejected the request: a bad key (401), or a body it would
    /// not accept (400/422).
    #[error("pioneer rejected the request ({status}): {body}")]
    Rejected { status: u16, body: String },
    /// The API answered, but not in a shape this backend can read.
    #[error("pioneer protocol error: {0}")]
    Protocol(String),
}

impl From<PioneerError> for Error {
    /// Map transport onto [`ErrorKind::Transport`], and anything the
    /// provider actually answered onto [`ErrorKind::Provider`].
    fn from(err: PioneerError) -> Self {
        let kind = match err {
            PioneerError::Transport(_) => ErrorKind::Transport,
            PioneerError::Rejected { .. } | PioneerError::Protocol(_) => ErrorKind::Provider,
        };
        Error::new(kind, err)
    }
}
