//! Error translation: `bentoml` errors → [`elide_core::Error`].
//!
//! Crate-private — the public API of every backend reports
//! [`elide_core::Error`]; this enum is the internal seam the
//! per-route helpers use before bubbling up.

use elide_core::{Error, ErrorKind};

/// Errors surfaced internally by the bento backends.
///
/// Two structural categories the consuming crate maps onto
/// [`ErrorKind`] when bubbling up: transport (HTTP / network /
/// client construction) and protocol (service answered but the
/// body did not match the contract — decode error, batch length
/// mismatch, …).
#[derive(Debug, thiserror::Error)]
pub(crate) enum BentoError {
    /// HTTP / transport failure — client construction, network
    /// I/O, status-code rejections.
    #[error("bento transport error: {0}")]
    Transport(#[from] bentoml::Error),
    /// Protocol failure — the service answered but the body did not
    /// match the contract.
    #[error("bento protocol error: {0}")]
    Protocol(String),
}

impl From<BentoError> for Error {
    /// Map transport to [`ErrorKind::Transport`] and protocol to
    /// [`ErrorKind::Validation`], carrying the original error as the
    /// source cause.
    fn from(err: BentoError) -> Self {
        let kind = match err {
            BentoError::Transport(_) => ErrorKind::Transport,
            BentoError::Protocol(_) => ErrorKind::Validation,
        };
        Error::new(kind, err)
    }
}
