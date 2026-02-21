//! Conversion from [`object_store::Error`] to [`nvisy_core::Error`].

use nvisy_core::Error;

/// Convert an [`object_store::Error`] into a [`nvisy_core::Error`].
pub(crate) fn from_object_store(err: object_store::Error) -> Error {
    let retryable = !matches!(
        err,
        object_store::Error::NotFound { .. }
            | object_store::Error::PermissionDenied { .. }
            | object_store::Error::Unauthenticated { .. }
            | object_store::Error::AlreadyExists { .. }
            | object_store::Error::Precondition { .. }
    );
    Error::runtime(err.to_string(), "object-store", retryable)
        .with_source(err)
}
