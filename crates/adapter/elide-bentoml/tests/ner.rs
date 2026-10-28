//! NER adapter round trips against the real Python service.

#![cfg(feature = "ner")]

mod support;

use elide_bentoml::ner::BentoNer;
use elide_core::backend::Backend;
use elide_core::entity::{Label, LabelLocale, builtins};
use elide_core::primitive::LanguageTag;
use elide_ner::backend::NerRequest;
use support::{Server, status};

#[tokio::test]
#[ignore = "requires Docker; builds and runs real BentoML service containers"]
async fn ner_roundtrip_preserves_canonical_labels_unicode_and_batch_order() {
    let server = Server::start("ner").await;
    let backend = BentoNer::new(&server.url, "e2e/ner").unwrap();

    // Use the actual built-in: its display name differs from its canonical ID.
    let labels = [builtins::PERSON_NAME.clone()];
    let polish: LanguageTag = "pl".parse().unwrap();
    let localized = [labels[0]
        .clone()
        .with_localization(polish.clone(), LabelLocale::new("osoba"))];
    let request = |labels, language| NerRequest {
        text: "Łódź Ada",
        labels: Some(labels),
        language,
        correlation_id: None,
    };
    let response = backend.call(request(&labels, None)).await.unwrap();
    assert_eq!(response.spans[0].label, builtins::PERSON_NAME.to_ref());
    assert_eq!(response.spans[0].offset, 8..11);

    let responses = backend
        .call_batch(vec![
            request(&localized, Some(&polish)),
            request(&[], None),
            request(&labels, None),
        ])
        .await
        .unwrap();
    assert_eq!(responses.len(), 3);
    assert_eq!(responses[0].spans[0].label, labels[0].to_ref());
    assert!(responses[1].spans.is_empty());
    assert_eq!(responses[2].spans[0].offset, 8..11);
    assert_eq!(responses[2].spans[0].label, labels[0].to_ref());

    // Threshold must survive Rust serialization and Python parsing.
    let strict = backend.clone().with_default_threshold(0.99);
    assert!(
        strict
            .call(request(&labels, None))
            .await
            .unwrap()
            .spans
            .is_empty()
    );

    let long = NerRequest {
        text: "one two three four five six",
        labels: Some(&labels),
        language: None,
        correlation_id: None,
    };
    assert_eq!(status(&backend.call(long).await.unwrap_err()), Some(400));
    let large_schema = [Label::new("person_name", "person name").with_localization(
        LanguageTag::english(),
        LabelLocale::described("person name", "x".repeat(2000)),
    )];
    assert_eq!(
        status(
            &backend
                .call(request(&large_schema, None))
                .await
                .unwrap_err()
        ),
        Some(400)
    );
    let failure = NerRequest {
        text: "model failure",
        labels: Some(&labels),
        language: None,
        correlation_id: None,
    };
    assert_eq!(status(&backend.call(failure).await.unwrap_err()), Some(500));
}
