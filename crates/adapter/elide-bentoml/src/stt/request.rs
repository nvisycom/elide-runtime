//! Outgoing wire types for the STT `/transcribe` endpoint.
//!
//! Mirrors `bento_core.stt.v1.SttRequest` from the inference
//! repository: base64-encoded audio bytes plus an optional filename
//! and language hint.

use base64::Engine;
use base64::engine::general_purpose::STANDARD as BASE64;
use elide_audio::stt::SttRequest;
use serde::Serialize;

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
