//! A policy whose operator elide refuses is rejected when the
//! request compiles, naming the policy, rather than redacting with
//! an operator that would leave the region readable.

use elide::Report;
use elide::entity::LabelRef;
use elide_provider::{Deployment, DocumentContext, Pass, Provider};
use elide_schema::policy::{LabelScope, Policy};
use elide_schema::redaction::{AudioRedaction, ImageRedaction, ModalityRedactions};
use uuid::Uuid;

/// A policy redacting every face it scopes with `redactions`.
fn policy(redactions: ModalityRedactions) -> Policy {
    Policy {
        id: Uuid::now_v7(),
        name: "media".into(),
        scopes: vec![LabelScope::new("faces", [LabelRef::new("face")])],
        fallback: Some(redactions),
        ..Policy::default()
    }
}

/// The error compiling `policy` for anonymize raises.
fn compile_error(policy: &Policy) -> String {
    Provider::new(Deployment::default())
        .anonymize_orchestrator(
            &Pass {
                policies: std::slice::from_ref(policy),
                context: &DocumentContext::default(),
                recognition: &[],
                correlation_id: Uuid::now_v7(),
            },
            &Report::new(),
            None,
        )
        .err()
        .expect("the operator is refused")
        .to_string()
}

#[test]
fn a_blur_too_light_to_hide_anything_is_refused() {
    let policy = policy(ModalityRedactions::image(ImageRedaction::Blur {
        sigma: 0.5,
    }));
    let err = compile_error(&policy);
    assert!(
        err.contains(&policy.id.to_string()),
        "names the policy: {err}"
    );
}

#[test]
fn a_one_pixel_mosaic_is_refused() {
    let policy = policy(ModalityRedactions::image(ImageRedaction::Pixelate {
        block_size: 1,
    }));
    let err = compile_error(&policy);
    assert!(
        err.contains(&policy.id.to_string()),
        "names the policy: {err}"
    );
}

#[test]
fn a_beep_with_no_audible_frequency_is_refused() {
    let beep: AudioRedaction =
        serde_json::from_str(r#"{"kind": "beep", "hz": 0.0}"#).expect("a beep spec");
    let policy = policy(ModalityRedactions::audio(beep));
    let err = compile_error(&policy);
    assert!(
        err.contains(&policy.id.to_string()),
        "names the policy: {err}"
    );
}
