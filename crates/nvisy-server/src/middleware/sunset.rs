//! Sunset deprecation header middleware.
//!
//! Adds `Sunset`, `Deprecation`, and `Link` HTTP headers to responses
//! from deprecated API versions, signalling to clients that the version
//! will be removed after a specified date.
//!
//! Headers follow [RFC 8594](https://httpwg.org/specs/rfc8594.html) and
//! the [Deprecation header draft](https://datatracker.ietf.org/doc/draft-ietf-httpapi-deprecation-header/).

use axum::body::Body;
use axum::http::{HeaderValue, Request};
use axum::middleware::Next;
use axum::response::Response;

/// Configuration for the sunset deprecation middleware.
#[derive(Clone)]
pub struct SunsetConfig {
    /// HTTP-date when the version will be removed
    /// (e.g. `"Sat, 01 Nov 2025 00:00:00 GMT"`).
    pub sunset_date: HeaderValue,
    /// Link header pointing to the successor version
    /// (e.g. `</api/v2>; rel="successor-version"`).
    pub successor_link: HeaderValue,
}

impl SunsetConfig {
    /// Create a new config with the given sunset date and successor path.
    ///
    /// # Panics
    ///
    /// Panics if `sunset_date` or `successor_path` contain invalid header characters.
    pub fn new(sunset_date: &str, successor_path: &str) -> Self {
        Self {
            sunset_date: HeaderValue::from_str(sunset_date)
                .expect("sunset_date must be a valid header value"),
            successor_link: HeaderValue::from_str(&format!(
                "<{successor_path}>; rel=\"successor-version\""
            ))
            .expect("successor_path must produce a valid header value"),
        }
    }
}

/// Axum middleware function that adds sunset deprecation headers.
///
/// Use with [`axum::middleware::from_fn_with_state`] or inject the
/// config via a layer extension.
///
/// # Example
///
/// ```rust,ignore
/// use axum::middleware;
///
/// let config = SunsetConfig::new("Sat, 01 Nov 2025 00:00:00 GMT", "/api/v2");
/// let deprecated = v1::routes()
///     .layer(axum::Extension(config))
///     .layer(middleware::from_fn(sunset_headers));
/// ```
pub async fn sunset_headers(
    config: axum::extract::Extension<SunsetConfig>,
    req: Request<Body>,
    next: Next,
) -> Response {
    let mut response = next.run(req).await;
    let headers = response.headers_mut();
    headers.insert("sunset", config.sunset_date.clone());
    headers.insert("deprecation", HeaderValue::from_static("true"));
    headers.append("link", config.successor_link.clone());
    response
}
