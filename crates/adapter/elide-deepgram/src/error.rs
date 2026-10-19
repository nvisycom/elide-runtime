//! Error translation: [`deepgram::DeepgramError`] → [`elide_core::Error`].
//!
//! Crate-private — the public API reports [`elide_core::Error`]; this is
//! the internal seam the backend uses before bubbling up.

use deepgram::DeepgramError;
use elide_core::{Error, ErrorKind};

/// Errors surfaced internally by the Deepgram backend.
#[derive(Debug, thiserror::Error)]
pub(crate) enum DeepgramBackendError {
    /// The SDK failed: transport, decode, or a rejected request.
    #[error("deepgram error: {0}")]
    Sdk(#[from] DeepgramError),
    /// The API answered, but not with a transcript this backend can use.
    #[error("deepgram returned no usable transcript: {0}")]
    Protocol(String),
}

impl From<DeepgramBackendError> for Error {
    /// Map anything the network did onto [`ErrorKind::Transport`], and
    /// anything the provider actually answered onto [`ErrorKind::Provider`].
    fn from(err: DeepgramBackendError) -> Self {
        let kind = match &err {
            DeepgramBackendError::Sdk(inner) => match inner {
                // Transport-shaped: the request never got a usable answer.
                DeepgramError::ReqwestError(_)
                | DeepgramError::HttpError(_)
                | DeepgramError::IoError(_)
                | DeepgramError::InvalidUrl => ErrorKind::Transport,
                // Everything else is the provider answering badly: an API
                // error body, or a response that would not decode.
                _ => ErrorKind::Provider,
            },
            DeepgramBackendError::Protocol(_) => ErrorKind::Provider,
        };
        Error::new(kind, err)
    }
}
