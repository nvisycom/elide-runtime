//! API version extractor.
//!
//! Parses the version number from the request URI path (`/api/v{n}/...`)
//! and provides it as a typed [`ApiVersion`] newtype.

use std::num::NonZeroU16;

use aide::OperationInput;
use axum::extract::FromRequestParts;
use axum::http::request::Parts;

/// API version number extracted from the request path.
///
/// Contains `Some(n)` when the path matches `/api/v{n}/...`, or `None`
/// for unversioned paths (e.g. `/health`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ApiVersion(pub Option<NonZeroU16>);

impl ApiVersion {
    /// The version number, if present.
    pub fn version(&self) -> Option<u16> {
        self.0.map(|v| v.get())
    }
}

impl OperationInput for ApiVersion {}

impl<S: Send + Sync> FromRequestParts<S> for ApiVersion {
    type Rejection = std::convert::Infallible;

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
        assert_eq!(ApiVersion::from_path("/api/v1/runs").version(), Some(1));
        assert_eq!(ApiVersion::from_path("/api/v2/files").version(), Some(2));
        assert_eq!(
            ApiVersion::from_path("/api/v12/contexts").version(),
            Some(12)
        );
        assert_eq!(ApiVersion::from_path("/api/v1").version(), Some(1));
    }

    #[test]
    fn returns_none_for_non_api_paths() {
        assert_eq!(ApiVersion::from_path("/health").version(), None);
        assert_eq!(ApiVersion::from_path("/api/runs").version(), None);
        assert_eq!(ApiVersion::from_path("/api/vx/runs").version(), None);
    }

    #[test]
    fn rejects_version_zero() {
        assert_eq!(ApiVersion::from_path("/api/v0/runs").version(), None);
    }
}
