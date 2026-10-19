//! Rate limiting reaching the backend decorator stack.
//!
//! The config knob is tested in `elide-deployment`; what matters
//! here is that a configured limit compiles into a working
//! analyzer, and that an unlimited component still compiles through
//! the same path.

use elide::ErrorKind;
use elide::entity::LabelRef;
use elide_provider::deployment::RateLimit;
use elide_provider::{
    Availability, Component, ComponentSelection, Deployment, DocumentContext, Enrichers,
    NerBackend, Provider, Recognizers,
};
use elide_schema::policy::{LabelScope, Policy};

/// A policy naming one label, so compilation reaches the backend
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

/// Compile an analyzer over one mock NER component carrying
/// `rate_limit`.
fn compile_with(rate_limit: Option<RateLimit>) -> Result<(), (ErrorKind, String)> {
    let mut component = Component::new("limited", NerBackend::Mock);
    if let Some(limit) = rate_limit {
        component = component.with_rate_limit(limit);
    }
    let deployment = Deployment::try_from_parts(
        Recognizers {
            ner: vec![component],
            ..Recognizers::default()
        },
        Enrichers::default(),
        Availability::All,
    )
    .expect("the lineup is valid");
    let provider = Provider::new(deployment);

    provider
        .analyze_orchestrator(
            &DocumentContext::default(),
            &[],
            &[a_policy()],
            uuid::Uuid::now_v7(),
            &ComponentSelection::new(),
        )
        .map(|_| ())
        .map_err(|err| (err.kind(), err.to_string()))
}

/// A component with no limit compiles. It goes through the same
/// decorator stack as a limited one — the limiters are set wide
/// enough never to block — so there is one concrete backend type
/// per kind rather than two code paths.
#[test]
fn an_unlimited_component_compiles() {
    compile_with(None).expect("compiles");
}

/// A configured limit compiles into the stack. The decorators panic
/// on a zero rate, so reaching this point at all proves the
/// validated values are what got through.
#[test]
fn a_configured_limit_compiles_into_the_stack() {
    compile_with(Some(RateLimit {
        requests: 60,
        interval_seconds: 60,
        burst: Some(10),
        max_concurrent: Some(4),
    }))
    .expect("compiles");
}

/// A limit of one call per hour still compiles: the stack is built
/// at compile time and the bucket starts full, so construction does
/// not block on the rate.
#[test]
fn a_severe_limit_still_compiles() {
    compile_with(Some(RateLimit {
        requests: 1,
        interval_seconds: 3600,
        burst: None,
        max_concurrent: Some(1),
    }))
    .expect("compiles");
}
