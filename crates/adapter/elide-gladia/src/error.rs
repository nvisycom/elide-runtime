//! Error translation: [`gladia::Error`] → [`elide_core::Error`].
//!
//! Crate-private — the public API reports [`elide_core::Error`]; this is
//! the internal seam the backend uses before bubbling up.

use elide_core::{Error, ErrorKind};

/// Errors surfaced internally by the Gladia backend.
#[derive(Debug, thiserror::Error)]
pub(crate) enum GladiaError {
    /// The SDK failed: transport, decode, or a rejected request.
    #[error("gladia error: {0}")]
    Sdk(#[from] gladia::Error),
    /// The job finished in a non-`done` state, or `done` without a body.
    #[error("gladia transcription failed: {0}")]
    Failed(String),
}

impl From<GladiaError> for Error {
    /// Map anything the network did onto [`ErrorKind::Transport`], and
    /// anything the provider actually answered onto [`ErrorKind::Provider`].
    fn from(err: GladiaError) -> Self {
        let kind = match &err {
            GladiaError::Sdk(inner) => match inner {
                gladia::Error::Transport(_) | gladia::Error::Timeout { .. } => ErrorKind::Transport,
                _ => ErrorKind::Provider,
            },
            GladiaError::Failed(_) => ErrorKind::Provider,
        };
        Error::new(kind, err)
    }
}
