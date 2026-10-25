//! Outgoing wire types for the STT `/transcribe` endpoint.
//!
//! Mirrors `elide_core.stt.v1.SttRequest` from the inference
//! repository: base64-encoded audio bytes plus an optional filename
//! and language hint.

use base64::Engine;
use base64::engine::general_purpose::STANDARD as BASE64;
use elide_audio::stt::SttRequest;
use serde::Serialize;

/// The `/transcribe` input model.
///
/// BentoML keys an endpoint's input on its handler's parameter names, so
/// `transcribe(self, request: SttRequest, ctx)` takes `{"request": {...}}`
/// and rejects a bare object with `400 1 validation error for Input`. The
/// field is `request` rather than `requests` because this endpoint is not
/// batching: the name mirrors the handler, not the payload's shape.
#[derive(Debug, Serialize)]
pub(super) struct Transcribe<'a> {
    /// Named for the handler parameter that receives it.
    pub request: &'a WireSttRequest,
}

/// Outgoing per-call request body.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct WireSttRequest {
    /// Base64-encoded audio bytes.
    pub audio: String,
    /// The service uses this for container detection, and accepts its
    /// absence. Always `None` here: [`SttRequest`] carries only bytes, so
    /// there is no name to send, and the service sniffs the payload.
    ///
    /// [`SttRequest`]: elide_audio::stt::SttRequest
    #[serde(skip_serializing_if = "Option::is_none")]
    pub filename: Option<String>,
    /// Caller-asserted language as a BCP-47 tag, when supplied.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub language: Option<String>,
}

impl WireSttRequest {
    pub(super) fn from_request(request: &SttRequest<'_>) -> Self {
        Self {
            audio: BASE64.encode(request.audio),
            filename: None,
            language: request.language.map(ToString::to_string),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The body must be the object the service's OpenAPI declares.
    ///
    /// Posting `WireSttRequest` directly compiles — both it and [`Transcribe`]
    /// are `Serialize`, so `invoke` accepts either — and is the production bug
    /// this wrapper exists to fix. Without this assertion the whole suite
    /// passes with the envelope dropped, which is how it shipped.
    ///
    /// The assertion is on the key's presence rather than the exact field
    /// name: renaming `request` is a change the compiler demands in both this
    /// module and the call site, so pinning the spelling would only restate
    /// the struct.
    #[test]
    fn the_body_is_an_object_rather_than_the_bare_request() {
        let request = WireSttRequest {
            audio: "AAAA".to_owned(),
            filename: None,
            language: None,
        };

        let json =
            serde_json::to_value(Transcribe { request: &request }).expect("the request serializes");

        assert!(
            json.get("request").is_some_and(|r| r.is_object()),
            "`/transcribe` declares `required: [\"request\"]`, got {json}"
        );
    }
}
