//! [`ExifModule`]: EXIF extraction via the Python bridge.

use nvisy_core::Error;
use nvisy_core::content::Content;
use pyo3::prelude::*;
use pyo3::types::PyDict;
use serde_json::Value;

use super::params::ExifParams;
use crate::bridge::{PythonBridge, from_pyerr};

const TARGET: &str = "nvisy_python::exif";

/// Configured handle for EXIF metadata extraction.
///
/// Holds a [`PythonBridge`] and [`ExifParams`] so callers do not need
/// to pass them on every invocation.
#[derive(Debug, Clone)]
pub struct ExifModule {
    /// Python bridge used to call into the `nvisy_ai` module.
    bridge: PythonBridge,
    /// Extraction parameters applied to every call.
    params: ExifParams,
}

impl ExifModule {
    /// Creates a new module with the given bridge and parameters.
    pub fn new(bridge: PythonBridge, params: ExifParams) -> Self {
        Self { bridge, params }
    }

    /// Returns a reference to the underlying bridge.
    #[must_use]
    pub fn bridge(&self) -> &PythonBridge {
        &self.bridge
    }

    /// Returns a reference to the current parameters.
    #[must_use]
    pub fn params(&self) -> &ExifParams {
        &self.params
    }

    /// Calls Python `extract_exif()` synchronously via `spawn_blocking`.
    ///
    /// Returns raw JSON dicts containing EXIF tag key-value pairs.
    /// The MIME type is resolved from `content.content_type()`,
    /// defaulting to `"application/octet-stream"` when unavailable.
    ///
    /// # Errors
    ///
    /// Returns an error if the Python call fails or the return value
    /// cannot be deserialized.
    #[tracing::instrument(
        target = TARGET,
        name = "exif.extract",
        skip(self, content),
        fields(data_len = content.size()),
    )]
    pub async fn extract(&self, content: Content) -> Result<Vec<Value>, Error> {
        let request = ExifRequest::new(content, self.params);

        self.bridge
            .call_sync("extract_exif", move |py| request.to_kwargs(py))
            .await
    }

    /// Calls Python `extract_exif()` as a **coroutine** (async Python
    /// function).
    ///
    /// Returns raw JSON dicts containing EXIF tag key-value pairs.
    /// The MIME type is resolved from `content.content_type()`,
    /// defaulting to `"application/octet-stream"` when unavailable.
    ///
    /// # Errors
    ///
    /// Returns an error if the Python call fails or the return value
    /// cannot be deserialized.
    #[tracing::instrument(
        target = TARGET,
        name = "exif.extract_async",
        skip(self, content),
        fields(data_len = content.size()),
    )]
    pub async fn extract_async(&self, content: Content) -> Result<Vec<Value>, Error> {
        let request = ExifRequest::new(content, self.params);

        self.bridge
            .call_async("extract_exif", move |py| request.to_kwargs(py))
            .await
    }
}

/// Owned snapshot of a single EXIF extraction request.
///
/// Wraps [`Content`] and [`ExifParams`] so they can be moved into
/// a `Send + 'static` closure for the bridge call.
struct ExifRequest {
    /// Content to extract EXIF metadata from.
    content: Content,
    /// Extraction parameters.
    params: ExifParams,
}

impl ExifRequest {
    fn new(content: Content, params: ExifParams) -> Self {
        Self { content, params }
    }

    /// Converts the request into a Python keyword arguments dict.
    fn to_kwargs<'py>(&self, py: Python<'py>) -> Result<Bound<'py, PyDict>, Error> {
        let mime_type = self
            .content
            .content_type()
            .unwrap_or("application/octet-stream");

        let kwargs = PyDict::new(py);
        kwargs
            .set_item("image_bytes", self.content.as_bytes())
            .map_err(from_pyerr)?;
        kwargs
            .set_item("mime_type", mime_type)
            .map_err(from_pyerr)?;
        kwargs
            .set_item("include_gps", self.params.include_gps)
            .map_err(from_pyerr)?;
        kwargs
            .set_item("include_thumbnail", self.params.include_thumbnail)
            .map_err(from_pyerr)?;
        Ok(kwargs)
    }
}
