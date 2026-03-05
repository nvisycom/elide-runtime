//! Conversion from [`nvisy_core::Error`] to HTTP [`Error`].

use super::http_error::Error;
use super::http_kind::ErrorKind;

impl From<nvisy_core::Error> for Error<'static> {
    fn from(err: nvisy_core::Error) -> Self {
        let kind = match err.kind {
            nvisy_core::ErrorKind::Validation | nvisy_core::ErrorKind::Serialization => {
                ErrorKind::BadRequest
            }
            nvisy_core::ErrorKind::Policy => ErrorKind::Forbidden,
            nvisy_core::ErrorKind::NotFound => ErrorKind::NotFound,
            nvisy_core::ErrorKind::Connection
            | nvisy_core::ErrorKind::Timeout
            | nvisy_core::ErrorKind::Cancellation
            | nvisy_core::ErrorKind::Runtime
            | nvisy_core::ErrorKind::Internal => ErrorKind::InternalServerError,
        };

        let mut error = Self::new(kind).with_message(err.message);
        if let Some(component) = err.source_component {
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
        let core_err =
            nvisy_core::Error::new(nvisy_core::ErrorKind::Validation, "field is required");
        let err = Error::from(core_err);
        assert_eq!(err.kind(), ErrorKind::BadRequest);
        assert_eq!(err.message(), Some("field is required"));
    }

    #[test]
    fn from_nvisy_core_not_found() {
        let core_err = nvisy_core::Error::new(nvisy_core::ErrorKind::NotFound, "missing")
            .with_component("registry");
        let err = Error::from(core_err);
        assert_eq!(err.kind(), ErrorKind::NotFound);
        assert_eq!(err.message(), Some("missing"));
        assert_eq!(err.context(), Some("registry"));
    }

    #[test]
    fn from_nvisy_core_internal() {
        let core_err = nvisy_core::Error::new(nvisy_core::ErrorKind::Runtime, "unexpected");
        let err = Error::from(core_err);
        assert_eq!(err.kind(), ErrorKind::InternalServerError);
    }
}
