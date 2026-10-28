//! STT adapter round trips against the real Python service.

#![cfg(feature = "stt")]

mod support;

use elide_audio::fixtures::wav_ramp;
use elide_audio::modality::AudioFormat;
use elide_audio::stt::SttRequest;
use elide_bentoml::stt::BentoStt;
use elide_core::backend::Backend;
use elide_core::primitive::LanguageTag;
use support::{Server, status};

#[tokio::test]
#[ignore = "requires Docker; builds and runs real BentoML service containers"]
async fn stt_roundtrip_decodes_audio_and_preserves_milliseconds_and_words() {
    let server = Server::start("stt").await;
    let backend = BentoStt::new(&server.url, "e2e/stt").unwrap();
    let language: LanguageTag = "pl".parse().unwrap();
    let request = |audio| SttRequest {
        audio,
        format: AudioFormat::Wav,
        language: Some(&language),
        correlation_id: None,
    };
    let audio = wav_ramp(1);
    let response = backend.call(request(&audio)).await.unwrap();
    assert_eq!(response.segments.len(), 1);
    let segment = &response.segments[0];
    assert_eq!(segment.text, "Ada");
    assert_eq!(segment.language.as_ref(), Some(&language));
    assert_eq!(
        (segment.span.start_millis(), segment.span.end_millis()),
        (125, 375)
    );
    assert_eq!(segment.words.len(), 1);
    assert_eq!(segment.words[0].text, "Ada");
    assert_eq!(
        (
            segment.words[0].span.start_millis(),
            segment.words[0].span.end_millis()
        ),
        (125, 375)
    );
    assert!(segment.words[0].confidence.is_some());
    assert_eq!(
        status(&backend.call(request(b"not audio")).await.unwrap_err()),
        Some(400)
    );
    assert_eq!(
        status(&backend.call(request(&[])).await.unwrap_err()),
        Some(400)
    );
    let long_audio = wav_ramp(2);
    let oversized = vec![0; 100_001];
    assert_eq!(
        status(&backend.call(request(&long_audio)).await.unwrap_err()),
        Some(400)
    );
    assert_eq!(
        status(&backend.call(request(&oversized)).await.unwrap_err()),
        Some(400)
    );
}
