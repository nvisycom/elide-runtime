//! Backends whose client must be built before a request arrives.
//!
//! Google Document AI's client builder is `async`, and the
//! per-request orchestrator compile is not, so its client is built
//! once by `Provider::try_new`. What matters is that a provider
//! built the synchronous way *refuses* such a deployment rather
//! than quietly running without the enricher: a document that
//! silently got no OCR reads exactly like one whose images hold no
//! text.

#![cfg(feature = "google-document-ai")]

use elide::ErrorKind;
use elide::entity::LabelRef;
use elide_provider::{
    Availability, Component, ComponentSelection, Deployment, DocumentContext, Enrichers,
    OcrBackend, Provider, Recognizers,
};
use elide_schema::policy::{LabelScope, Policy};

/// A policy naming one label, so compilation reaches the enricher
/// wiring rather than stopping at "nothing to detect".
fn a_policy() -> Policy {
    Policy {
        id: uuid::Uuid::now_v7(),
        name: "detect-email".into(),
        scopes: vec![LabelScope::new(
            "contact",
            vec![LabelRef::new("email_address")],
        )],
        ..Policy::default()
    }
}

/// A deployment naming Document AI.
fn deployment() -> Deployment {
    Deployment::try_from_parts(
        Recognizers::default(),
        Enrichers {
            ocr: vec![Component::new(
                "gcp-ocr",
                OcrBackend::GoogleDocumentAi {
                    processor: "projects/p/locations/eu/processors/abc".to_owned(),
                    endpoint: None,
                },
            )],
            ..Enrichers::default()
        },
        Availability::All,
    )
    .expect("one component is a valid lineup")
}

/// Built synchronously, the deployment compiles no analyzer: the
/// client was never awaited, and running without the enricher the
/// deployment asked for would be a silent downgrade.
#[test]
fn the_sync_constructor_refuses_rather_than_skipping() {
    let provider = Provider::new(deployment());

    let result = provider.analyze_orchestrator(
        &DocumentContext::default(),
        &[],
        &[a_policy()],
        uuid::Uuid::now_v7(),
        &ComponentSelection::new(),
    );
    let Err(err) = result else {
        panic!("must refuse: the Document AI client cannot exist yet");
    };

    assert_eq!(err.kind(), ErrorKind::Configuration);
    let message = err.to_string();
    assert!(
        message.contains("gcp-ocr"),
        "the message must name the component: {message}",
    );
    assert!(
        message.contains("try_new"),
        "and say how to fix it: {message}",
    );
}

/// Two Document AI components sharing a name are refused before a
/// client is ever built.
///
/// Now caught by `Deployment::try_from_parts`, since a deployment
/// cannot hold a duplicated lineup name at all. The client cache
/// keeps its own check regardless — it is a map keyed by component
/// name, and silently overwriting would run OCR against the
/// *other* component's processor (different model version, region,
/// possibly trust boundary) with no error anywhere. That check is
/// unreachable while this constructor is the only way in, which is
/// the right shape: defence where the invariant is relied upon,
/// enforcement where the value is made.
#[test]
fn duplicate_names_cannot_reach_the_client_cache() {
    let one = |processor: &str| {
        Component::new(
            "gcp-ocr",
            OcrBackend::GoogleDocumentAi {
                processor: processor.to_owned(),
                endpoint: None,
            },
        )
    };

    let result = Deployment::try_from_parts(
        Recognizers::default(),
        Enrichers {
            ocr: vec![
                one("projects/p/locations/eu/processors/first"),
                one("projects/p/locations/us/processors/second"),
            ],
            ..Enrichers::default()
        },
        Availability::All,
    );

    let Err(err) = result else {
        panic!("two components named `gcp-ocr` must be refused, not collapsed");
    };
    assert!(
        err.message().contains("gcp-ocr"),
        "the message must name the collision: {err}",
    );
}
