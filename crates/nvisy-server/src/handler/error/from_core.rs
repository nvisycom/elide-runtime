//! Conversion from [`CoreError`] to HTTP [`Error`].
//!
//! [`CoreError`]: nvisy_core::Error

use nvisy_core::{Error as CoreError, ErrorKind as CoreErrorKind};

use super::http_error::Error;
use super::http_kind::ErrorKind;

impl From<CoreError> for Error<'static> {
    fn from(err: CoreError) -> Self {
        let kind = match err.kind() {
            CoreErrorKind::Validation => {
                if err.component() == Some("run") {
                    ErrorKind::Conflict
                } else {
                    ErrorKind::BadRequest
                }
            }
            CoreErrorKind::Serialization => ErrorKind::BadRequest,
            CoreErrorKind::Policy => ErrorKind::Forbidden,
            CoreErrorKind::NotFound => ErrorKind::NotFound,
            CoreErrorKind::Connection
            | CoreErrorKind::Timeout
            | CoreErrorKind::Cancellation
            | CoreErrorKind::Runtime
            | CoreErrorKind::Internal => ErrorKind::InternalServerError,
        };

        let component = err.component().map(str::to_owned);
        let mut error = Self::new(kind).with_message(err.message().to_owned());
        if let Some(component) = component {
            error = error.with_context(component);
        }
        error
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn from_nvisy_core_validation() {
        let core_err = CoreError::new(CoreErrorKind::Validation, "field is required");
        let err = Error::from(core_err);
        assert_eq!(err.kind(), ErrorKind::BadRequest);
        assert_eq!(err.message(), Some("field is required"));
    }

    #[test]
    fn from_nvisy_core_validation_run_conflict() {
        let core_err = CoreError::new(CoreErrorKind::Validation, "run has already finished")
            .with_component("run");
        let err = Error::from(core_err);
        assert_eq!(err.kind(), ErrorKind::Conflict);
    }

    #[test]
    fn from_nvisy_core_not_found() {
        let core_err =
            CoreError::new(CoreErrorKind::NotFound, "missing").with_component("registry");
        let err = Error::from(core_err);
        assert_eq!(err.kind(), ErrorKind::NotFound);
        assert_eq!(err.message(), Some("missing"));
        assert_eq!(err.context(), Some("registry"));
    }

    #[test]
    fn from_nvisy_core_internal() {
        let core_err = CoreError::new(CoreErrorKind::Runtime, "unexpected");
        let err = Error::from(core_err);
        assert_eq!(err.kind(), ErrorKind::InternalServerError);
    }
}
