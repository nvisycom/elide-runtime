use derive_more::Deref;
use reqwest_middleware::ClientWithMiddleware;

use nvisy_core::Error;

use super::ImageInput;

/// HTTP client wrapper with shared helpers for OCR backends.
///
/// Wraps [`ClientWithMiddleware`] and provides convenience methods for
/// status checking, JSON parsing, and multipart image upload construction.
/// Derefs to the inner client for direct request building.
#[derive(Deref)]
pub(crate) struct HttpClient {
    #[deref]
    inner: ClientWithMiddleware,
}

impl HttpClient {
    /// Create a new HTTP client wrapper.
    pub fn new(client: ClientWithMiddleware) -> Self {
        Self { inner: client }
    }

    /// Check that a response has a success status code.
    ///
    /// On failure, consumes the response body and returns an [`Error`] with
    /// the status code and body text.
    pub async fn check_status(
        &self,
        resp: reqwest_middleware::reqwest::Response,
        name: &str,
        source: &str,
    ) -> Result<reqwest_middleware::reqwest::Response, Error> {
        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            return Err(Error::runtime(
                format!("{name} returned {status}: {body}"),
                source,
                false,
            ));
        }
        Ok(resp)
    }

    /// Parse a JSON response body into the given type.
    pub async fn parse_json<T: serde::de::DeserializeOwned>(
        &self,
        resp: reqwest_middleware::reqwest::Response,
        name: &str,
        source: &str,
    ) -> Result<T, Error> {
        resp.json().await.map_err(|e| {
            Error::runtime(
                format!("failed to parse {name} response: {e}"),
                source,
                false,
            )
        })
    }

    /// Build a multipart [`Part`](reqwest_middleware::reqwest::multipart::Part)
    /// from an [`ImageInput`].
    pub fn image_part(
        &self,
        image: &ImageInput,
    ) -> Result<reqwest_middleware::reqwest::multipart::Part, Error> {
        reqwest_middleware::reqwest::multipart::Part::bytes(image.data.to_vec())
            .file_name("image")
            .mime_str(image.mime_type())
            .map_err(|e| Error::runtime(format!("invalid mime type: {e}"), "ocr", false))
    }
}
