//! API version extractor.
//!
//! Parses the version number from the request URI path (`/api/v{n}/...`)
//! and provides it as a typed [`ApiVersion`] newtype.

use std::convert::Infallible;
use std::num::NonZeroU16;

use aide::OperationInput;
use axum::extract::FromRequestParts;
use axum::http::request::Parts;

/// API version number extracted from the request path.
///
/// Contains `Some(n)` when the path matches `/api/v{n}/...`, or `None`
/// for unversioned paths (e.g. `/health`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ApiVersion(Option<NonZeroU16>);

impl ApiVersion {
    /// The version number as a plain integer, if present.
    pub fn version(&self) -> Option<u16> {
        self.0.map(|v| v.get())
    }

    /// The version number as `NonZeroU16`, if present. Convenient
    /// when used as a key in maps that exclude zero.
    pub fn nonzero(&self) -> Option<NonZeroU16> {
        self.0
    }
}

impl OperationInput for ApiVersion {}

impl<S: Send + Sync> FromRequestParts<S> for ApiVersion {
    type Rejection = Infallible;

    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        Ok(Self::from_path(parts.uri.path()))
    }
}

impl ApiVersion {
    /// Parse the version from a URI path.
    fn from_path(path: &str) -> Self {
        let version = path
            .strip_prefix("/api/v")
            .and_then(|rest| {
                let end = rest.find('/').unwrap_or(rest.len());
                rest[..end].parse::<u16>().ok()
            })
            .and_then(NonZeroU16::new);
        Self(version)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_version_from_path() {
        assert_eq!(
            ApiVersion::from_path("/api/v1/detections").version(),
            Some(1)
        );
        assert_eq!(ApiVersion::from_path("/api/v2/files").version(), Some(2));
        assert_eq!(
            ApiVersion::from_path("/api/v12/redactions").version(),
            Some(12)
        );
        assert_eq!(ApiVersion::from_path("/api/v1").version(), Some(1));
    }

    #[test]
    fn returns_none_for_non_api_paths() {
        assert_eq!(ApiVersion::from_path("/health").version(), None);
        assert_eq!(ApiVersion::from_path("/api/detections").version(), None);
        assert_eq!(ApiVersion::from_path("/api/vx/detections").version(), None);
    }

    #[test]
    fn rejects_version_zero() {
        assert_eq!(ApiVersion::from_path("/api/v0/detections").version(), None);
    }
}
