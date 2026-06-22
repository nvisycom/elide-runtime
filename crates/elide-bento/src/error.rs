//! Error translation: `bentoml` errors → [`elide_core::Error`].

use elide_core::{Error, ErrorKind};

/// Errors surfaced by [`BentoClient`](crate::BentoClient) operations.
///
/// Wraps the upstream [`bentoml::Error`] with a structural classification
/// the consuming crate can map onto the right [`ErrorKind`] when bubbling
/// up to elide.
#[derive(Debug, thiserror::Error)]
pub enum BentoError {
    /// HTTP / transport failure talking to the BentoML service.
    #[error("bento transport error: {0}")]
    Transport(#[from] bentoml::Error),
    /// Configuration failure (bad URL, missing required field, …).
    #[error("bento config error: {0}")]
    Config(String),
}

impl From<BentoError> for Error {
    /// Map a transport failure to [`ErrorKind::Transport`] and a config
    /// failure to [`ErrorKind::Validation`], carrying the original error
    /// as the source cause.
    fn from(err: BentoError) -> Self {
        let kind = match err {
            BentoError::Transport(_) => ErrorKind::Transport,
            BentoError::Config(_) => ErrorKind::Validation,
        };
        Error::new(kind, err)
    }
}
