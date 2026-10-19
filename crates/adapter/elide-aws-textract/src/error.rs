//! Error translation: Textract failures → [`elide_core::Error`].
//!
//! Crate-private — the public API reports [`elide_core::Error`]; this is
//! the internal seam the backend uses before bubbling up.

use aws_sdk_textract::error::SdkError;
use elide_core::{Error, ErrorKind};

/// Errors surfaced internally by the Textract backend.
#[derive(Debug, thiserror::Error)]
pub(crate) enum TextractError {
    /// The call never reached the service: a timeout, or a failure to
    /// dispatch the request.
    #[error("textract transport error: {0}")]
    Transport(String),
    /// The service answered, and the answer was a failure.
    #[error("textract error: {0}")]
    Sdk(String),
    /// The API answered, but not with blocks this backend can read.
    #[error("textract returned no usable blocks: {0}")]
    Protocol(String),
}

impl TextractError {
    /// Classify an [`SdkError`] before it is flattened to a string.
    ///
    /// [`SdkError`] discriminates a call that never got an answer from one
    /// the service rejected, and the two are not equivalent to a caller:
    /// the first may be safe to retry, the second would fail the same way.
    pub(crate) fn from_sdk<E, R>(err: &SdkError<E, R>) -> Self
    where
        SdkError<E, R>: std::fmt::Display,
    {
        match err {
            // Construction failures are a malformed request, not transport:
            // the bytes never left because they could not be assembled.
            SdkError::TimeoutError(_) | SdkError::DispatchFailure(_) => {
                Self::Transport(err.to_string())
            }
            _ => Self::Sdk(err.to_string()),
        }
    }
}

impl From<TextractError> for Error {
    /// A call that never reached the service maps onto
    /// [`ErrorKind::Transport`]; anything the service answered, onto
    /// [`ErrorKind::Provider`].
    fn from(err: TextractError) -> Self {
        let kind = match &err {
            TextractError::Transport(_) => ErrorKind::Transport,
            TextractError::Sdk(_) | TextractError::Protocol(_) => ErrorKind::Provider,
        };
        Error::new(kind, err)
    }
}
