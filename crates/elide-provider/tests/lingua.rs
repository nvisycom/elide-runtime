//! Language-detection configuration reaching the enricher.
//!
//! The deployment knob exists because elide offers
//! `LinguaEnricher::with_candidates` and a deployment previously
//! had no way to state it: every build ran unrestricted detection.
//! What matters here is that a configured candidate set is honoured
//! and a malformed one is refused rather than silently ignored.

use elide::ErrorKind;
use elide::entity::LabelRef;
use elide_provider::deployment::Lingua;
use elide_provider::{Availability, Deployment, DocumentContext, Enrichers, Provider, Recognizers};
use elide_schema::policy::{LabelScope, Policy};

/// A policy naming one label, so compilation gets past the
/// "nothing to detect" check and reaches the enricher wiring.
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

/// Compiling an analyzer is what consults the lingua config, so
/// these resolve a request against a provider and read the outcome.
fn compile_with(lingua: Lingua) -> Result<(), (ErrorKind, String)> {
    let deployment = Deployment::try_from_parts(
        Recognizers::default(),
        Enrichers {
            lingua,
            ..Enrichers::default()
        },
        Availability::All,
    )
    .expect("the lineups are valid");
    let provider = Provider::new(deployment);

    provider
        .analyze_orchestrator(
            &DocumentContext::default(),
            &[],
            &[a_policy()],
            uuid::Uuid::now_v7(),
            &Default::default(),
        )
        .map(|_| ())
        .map_err(|err| (err.kind(), err.to_string()))
}

/// A deployment that configures no candidates compiles, and leaves
/// detection unrestricted — the behaviour every build had before
/// the knob existed.
#[test]
fn an_unconfigured_deployment_leaves_detection_unrestricted() {
    let lingua = Lingua::default();
    assert!(lingua.is_unrestricted(), "the default narrows nothing");
    compile_with(lingua).expect("compiles");
}

/// Valid BCP-47 tags reach the enricher. This is the capability the
/// knob exists for: a deployment that knows its corpus is English
/// and Spanish narrows detection to those two.
#[test]
fn configured_candidates_compile_into_the_enricher() {
    compile_with(Lingua {
        candidates: vec!["en".to_owned(), "es".to_owned()],
    })
    .expect("a two-language deployment compiles");
}

/// A tag that is not valid BCP-47 fails the compile, naming the
/// field. Silently dropping it would narrow detection to the tags
/// that happened to parse — or to none at all — and report success,
/// which reads exactly like a document in an undetected language.
#[test]
fn a_malformed_tag_is_refused_rather_than_dropped() {
    let (kind, message) = compile_with(Lingua {
        candidates: vec!["en".to_owned(), "not a language".to_owned()],
    })
    .expect_err("must not compile");

    assert_eq!(kind, ErrorKind::Configuration);
    assert!(
        message.contains("enrichers.lingua.candidates"),
        "the message must name the deployment field an operator has to fix: {message}",
    );
    assert!(
        message.contains("not a language"),
        "and the offending tag: {message}",
    );
}
