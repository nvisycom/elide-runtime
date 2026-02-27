//! `Accept-Version` header extractor.
//!
//! Parses a semver-like version from the `Accept-Version` request header.
//! Falls back to [`LATEST`](Version::LATEST) when the header is absent.

use std::fmt;
use std::num::NonZeroU32;
use std::str::FromStr;

use aide::OperationInput;
use axum::extract::FromRequestParts;
use axum::http::StatusCode;
use axum::http::request::Parts;

/// API version extracted from the `Accept-Version` header.
///
/// The version follows a simplified semver format: `major.minor.patch`.
/// When the header is missing the server assumes [`LATEST`](Version::LATEST).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Version {
    pub major: NonZeroU32,
    pub minor: u32,
    pub patch: u32,
}

impl Version {
    /// The latest (and currently only) API version.
    pub const LATEST: Self = Self {
        major: NonZeroU32::new(1).unwrap(),
        minor: 0,
        patch: 0,
    };

    /// Header name used to transmit the desired API version.
    const HEADER: &str = "Accept-Version";
}

impl fmt::Display for Version {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}.{}.{}", self.major, self.minor, self.patch)
    }
}

impl FromStr for Version {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let parts: Vec<&str> = s.trim().split('.').collect();
        match parts.len() {
            1 => {
                let major = parts[0]
                    .parse::<NonZeroU32>()
                    .map_err(|e| format!("invalid major version: {e}"))?;
                Ok(Self {
                    major,
                    minor: 0,
                    patch: 0,
                })
            }
            2 => {
                let major = parts[0]
                    .parse::<NonZeroU32>()
                    .map_err(|e| format!("invalid major version: {e}"))?;
                let minor = parts[1]
                    .parse::<u32>()
                    .map_err(|e| format!("invalid minor version: {e}"))?;
                Ok(Self {
                    major,
                    minor,
                    patch: 0,
                })
            }
            3 => {
                let major = parts[0]
                    .parse::<NonZeroU32>()
                    .map_err(|e| format!("invalid major version: {e}"))?;
                let minor = parts[1]
                    .parse::<u32>()
                    .map_err(|e| format!("invalid minor version: {e}"))?;
                let patch = parts[2]
                    .parse::<u32>()
                    .map_err(|e| format!("invalid patch version: {e}"))?;
                Ok(Self {
                    major,
                    minor,
                    patch,
                })
            }
            _ => Err(format!("expected 1-3 version components, got {}", parts.len())),
        }
    }
}

impl<S: Send + Sync> FromRequestParts<S> for Version {
    type Rejection = (StatusCode, String);

    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        match parts.headers.get(Self::HEADER) {
            Some(value) => {
                let s = value.to_str().map_err(|_| {
                    (
                        StatusCode::BAD_REQUEST,
                        format!("invalid `{}` header value", Self::HEADER),
                    )
                })?;
                s.parse::<Version>().map_err(|e| {
                    (
                        StatusCode::BAD_REQUEST,
                        format!("invalid `{}` header: {e}", Self::HEADER),
                    )
                })
            }
            None => Ok(Self::LATEST),
        }
    }
}

impl OperationInput for Version {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_full_version() {
        let v: Version = "1.2.3".parse().unwrap();
        assert_eq!(v.major.get(), 1);
        assert_eq!(v.minor, 2);
        assert_eq!(v.patch, 3);
    }

    #[test]
    fn parse_major_only() {
        let v: Version = "2".parse().unwrap();
        assert_eq!(v.major.get(), 2);
        assert_eq!(v.minor, 0);
        assert_eq!(v.patch, 0);
    }

    #[test]
    fn parse_major_minor() {
        let v: Version = "1.5".parse().unwrap();
        assert_eq!(v.major.get(), 1);
        assert_eq!(v.minor, 5);
        assert_eq!(v.patch, 0);
    }

    #[test]
    fn parse_zero_major_rejected() {
        assert!("0.1.0".parse::<Version>().is_err());
    }

    #[test]
    fn parse_invalid_string() {
        assert!("abc".parse::<Version>().is_err());
    }

    #[test]
    fn parse_too_many_parts() {
        assert!("1.2.3.4".parse::<Version>().is_err());
    }

    #[test]
    fn display_roundtrip() {
        let v = Version::LATEST;
        let s = v.to_string();
        let parsed: Version = s.parse().unwrap();
        assert_eq!(v, parsed);
    }

    #[test]
    fn latest_is_1_0_0() {
        assert_eq!(Version::LATEST.major.get(), 1);
        assert_eq!(Version::LATEST.minor, 0);
        assert_eq!(Version::LATEST.patch, 0);
        assert_eq!(Version::LATEST.to_string(), "1.0.0");
    }

    #[tokio::test]
    async fn from_request_parts_missing_header() {
        let mut parts = axum::http::Request::builder()
            .body(())
            .unwrap()
            .into_parts()
            .0;
        let version = Version::from_request_parts(&mut parts, &()).await.unwrap();
        assert_eq!(version, Version::LATEST);
    }

    #[tokio::test]
    async fn from_request_parts_with_header() {
        let mut parts = axum::http::Request::builder()
            .header("Accept-Version", "2.1.0")
            .body(())
            .unwrap()
            .into_parts()
            .0;
        let version = Version::from_request_parts(&mut parts, &()).await.unwrap();
        assert_eq!(version.major.get(), 2);
        assert_eq!(version.minor, 1);
        assert_eq!(version.patch, 0);
    }

    #[tokio::test]
    async fn from_request_parts_invalid_header() {
        let mut parts = axum::http::Request::builder()
            .header("Accept-Version", "not-a-version")
            .body(())
            .unwrap()
            .into_parts()
            .0;
        let result = Version::from_request_parts(&mut parts, &()).await;
        assert!(result.is_err());
        let (status, _) = result.unwrap_err();
        assert_eq!(status, StatusCode::BAD_REQUEST);
    }
}
