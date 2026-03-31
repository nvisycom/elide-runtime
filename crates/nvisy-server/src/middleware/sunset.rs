//! Sunset deprecation header middleware.
//!
//! Adds `Sunset`, `Deprecation`, and `Link` HTTP headers to responses
//! from deprecated API versions, signalling to clients that the version
//! will be removed after a specified date.
//!
//! Headers follow [RFC 8594](https://httpwg.org/specs/rfc8594.html) and
//! the [Deprecation header draft](https://datatracker.ietf.org/doc/draft-ietf-httpapi-deprecation-header/).

use std::collections::HashMap;

use axum::body::Body;
use axum::extract::Extension;
use axum::http::{HeaderValue, Request};
use axum::middleware::Next;
use axum::response::Response;

/// Per-version sunset entry: when the version sunsets and where to go.
#[derive(Clone)]
struct SunsetEntry {
    sunset_date: HeaderValue,
    successor_link: HeaderValue,
}

/// Configuration for the sunset deprecation middleware.
///
/// Maps API version prefixes (e.g. `"/api/v1"`) to their sunset date
/// and successor version. Only versions present in the map receive
/// deprecation headers; active versions pass through unmodified.
///
/// # Example
///
/// ```rust,ignore
/// use axum::middleware;
///
/// let config = SunsetConfig::new()
///     .add_version("/api/v1", "Sat, 01 Nov 2025 00:00:00 GMT", "/api/v2");
///
/// let app = Router::new()
///     .nest("/api/v1", v1_routes())
///     .nest("/api/v2", v2_routes())
///     .layer(Extension(config))
///     .layer(middleware::from_fn(sunset_headers));
/// ```
#[derive(Clone, Default)]
pub struct SunsetConfig {
    versions: HashMap<String, SunsetEntry>,
}

impl SunsetConfig {
    /// Create an empty config with no deprecated versions.
    pub fn new() -> Self {
        Self::default()
    }

    /// Register a deprecated version.
    ///
    /// - `prefix`: the URI prefix to match (e.g. `"/api/v1"`)
    /// - `sunset_date`: HTTP-date when the version will be removed
    ///   (e.g. `"Sat, 01 Nov 2025 00:00:00 GMT"`)
    /// - `successor_path`: base path of the replacement version
    ///   (e.g. `"/api/v2"`)
    ///
    /// # Panics
    ///
    /// Panics if `sunset_date` or `successor_path` contain invalid header characters.
    pub fn add_version(
        mut self,
        prefix: &str,
        sunset_date: &str,
        successor_path: &str,
    ) -> Self {
        self.versions.insert(
            prefix.to_owned(),
            SunsetEntry {
                sunset_date: HeaderValue::from_str(sunset_date)
                    .expect("sunset_date must be a valid header value"),
                successor_link: HeaderValue::from_str(&format!(
                    "<{successor_path}>; rel=\"successor-version\""
                ))
                .expect("successor_path must produce a valid header value"),
            },
        );
        self
    }
}

/// Axum middleware function that adds sunset deprecation headers to
/// responses for deprecated API versions.
///
/// Matches the request URI against the configured version prefixes.
/// Requests that don't match any deprecated version pass through
/// without modification.
pub async fn sunset_headers(
    config: Extension<SunsetConfig>,
    req: Request<Body>,
    next: Next,
) -> Response {
    let path = req.uri().path().to_owned();
    let mut response = next.run(req).await;

    if let Some(entry) = config.versions.iter().find(|(prefix, _)| path.starts_with(prefix.as_str())) {
        let headers = response.headers_mut();
        headers.insert("sunset", entry.1.sunset_date.clone());
        headers.insert("deprecation", HeaderValue::from_static("true"));
        headers.append("link", entry.1.successor_link.clone());
    }

    response
}
