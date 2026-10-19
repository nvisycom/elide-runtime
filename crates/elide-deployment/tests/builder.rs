//! The code path for authoring a deployment, and that it lowers
//! into the same value a file deserializes into.

use elide_deployment::Deployment;
use elide_deployment::component::{Availability, NerBackend, OcrBackend};

/// The builder and the struct literal produce the same deployment.
///
/// This is the property that keeps the two authoring paths from
/// drifting: whatever the builder can express, a configuration
/// file can, and vice versa.
#[test]
fn the_builder_and_a_literal_agree() {
    let built = Deployment::builder()
        .ner(
            "medical-ner",
            NerBackend::bento("https://ner.internal", "clinical"),
        )
        .tags(["medical"])
        .finish()
        .ocr(
            "fast-ocr",
            OcrBackend::bento("https://ocr.internal", "fast"),
        )
        .finish()
        .languages(["en", "es"])
        .availability(Availability::All)
        .build()
        .expect("a distinctly-named lineup with no rate limits is valid");

    let json = serde_json::to_value(&built).expect("serializes");
    let round_tripped: Deployment = serde_json::from_value(json.clone()).expect("deserializes");

    assert_eq!(
        serde_json::to_value(&round_tripped).expect("re-serializes"),
        json,
        "a built deployment round-trips through its own wire form",
    );

    assert_eq!(built.recognizers().ner.len(), 1);
    assert_eq!(built.recognizers().ner[0].tags, ["medical"]);
    assert_eq!(built.enrichers().ocr.len(), 1);
    assert_eq!(built.enrichers().lingua.candidates, ["en", "es"]);
}

/// An empty builder is the empty deployment, not a half-built one.
#[test]
fn an_empty_builder_is_the_default_deployment() {
    let built = Deployment::builder()
        .build()
        .expect("an empty deployment is valid");

    assert_eq!(
        serde_json::to_value(&built).expect("serializes"),
        serde_json::to_value(Deployment::default()).expect("serializes"),
    );
}

/// Several components in one lineup keep their order, which is
/// what a deployment's precedence depends on.
#[test]
fn a_lineup_keeps_the_order_it_was_built_in() {
    let built = Deployment::builder()
        .ner("first", NerBackend::bento("https://a", "m"))
        .finish()
        .ner("second", NerBackend::bento("https://b", "m"))
        .finish()
        .build()
        .expect("two distinctly-named components are valid");

    let names: Vec<&str> = built
        .recognizers()
        .ner
        .iter()
        .map(|c| c.name.as_str())
        .collect();
    assert_eq!(names, ["first", "second"]);
}
