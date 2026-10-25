//! Outgoing wire types for the OCR `/recognize` endpoint.
//!
//! Mirrors `elide_core.ocr.v1.OcrRequest` from the inference
//! repository: base64-encoded image bytes plus a per-word
//! confidence floor the service applies before returning.

use base64::Engine;
use base64::engine::general_purpose::STANDARD as BASE64;
use elide_image::ocr::OcrRequest;
use serde::Serialize;

/// The `/recognize` input model.
///
/// BentoML keys an endpoint's input on its handler's parameter names, so
/// `recognize(self, requests: list[OcrRequest], ctx)` takes
/// `{"requests": [...]}` and rejects a bare array with
/// `400 1 validation error for Input`.
#[derive(Debug, Serialize)]
pub(super) struct Recognize<'a> {
    /// Named for the handler parameter that receives it.
    pub requests: &'a [WireOcrRequest],
}

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

#[cfg(test)]
mod tests {
    use super::*;

    /// As for NER: the batching endpoint takes an object, not an array.
    #[test]
    fn the_body_is_an_object_keyed_on_the_handler_parameter() {
        let requests: Vec<WireOcrRequest> = Vec::new();

        let json = serde_json::to_value(Recognize {
            requests: &requests,
        })
        .expect("the request serializes");

        assert!(
            json.get("requests").is_some_and(|r| r.is_array()),
            "`/recognize` declares `required: [\"requests\"]`, got {json}"
        );
    }
}
