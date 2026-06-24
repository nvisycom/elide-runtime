//! Outgoing wire types for the OCR `/recognize` endpoint.
//!
//! Mirrors `nvisy_core.ocr.v1.OcrRequest` from the inference
//! repository: base64-encoded image bytes plus a per-word
//! confidence floor the service applies before returning.

use base64::Engine;
use base64::engine::general_purpose::STANDARD as BASE64;
use elide_ocr::backend::OcrRequest;
use serde::Serialize;

/// Outgoing per-call request body element.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct WireOcrRequest {
    /// Base64-encoded image bytes.
    pub image: String,
    /// Drop per-word recognitions weaker than this; `0.0` keeps all.
    pub confidence_threshold: f32,
}

impl WireOcrRequest {
    pub(super) fn from_request(request: &OcrRequest<'_>, default_threshold: f32) -> Self {
        Self {
            image: BASE64.encode(request.image),
            confidence_threshold: default_threshold,
        }
    }
}
