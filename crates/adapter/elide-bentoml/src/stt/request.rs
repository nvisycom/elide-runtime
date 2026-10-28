//! Outgoing wire types for the STT `/transcribe` endpoint.
//!
//! Mirrors `elide_core.stt.v1.SttRequest` from the inference
//! repository: base64-encoded audio bytes plus an optional filename
//! and language hint.

use base64::Engine;
use base64::engine::general_purpose::STANDARD as BASE64;
use elide_audio::stt::SttRequest;

use super::wire::SttRequest as WireSttRequest;

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
    use elide_audio::modality::AudioFormat;
    use elide_core::primitive::LanguageTag;

    use super::*;
    use crate::stt::wire::SttInput;

    #[test]
    fn domain_requests_match_python_fixtures() {
        let cases: serde_json::Value = serde_json::from_str(include_str!(
            "../../../../../contracts/stt/v1/fixtures.json"
        ))
        .unwrap();
        for case in cases.as_array().unwrap() {
            let mut expected = case["request"].clone();
            let request = &expected["request"];
            let bytes = BASE64.decode(request["audio"].as_str().unwrap()).unwrap();
            let language = request["language"]
                .as_str()
                .map(|tag| tag.parse::<LanguageTag>().unwrap());
            let domain = SttRequest {
                audio: &bytes,
                format: AudioFormat::Wav,
                language: language.as_ref(),
                correlation_id: None,
            };
            let wire = SttInput {
                request: WireSttRequest::from_request(&domain),
            };
            expected["request"]
                .as_object_mut()
                .unwrap()
                .retain(|_, value| !value.is_null());
            assert_eq!(serde_json::to_value(wire).unwrap(), expected);
        }
    }
}
