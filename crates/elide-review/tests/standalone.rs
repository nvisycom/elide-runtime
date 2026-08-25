//! Everything a consumer can do with this crate and `elide` alone:
//! build a report, deserialize edits from a request body, validate
//! them, and land them on the report.
//!
//! No engine, no provider, no pipeline — if this file compiles, the
//! crate stands on its own.

use elide::Report;
use elide::entity::audit::{AuditEvent, AuditKind, AuditLog};
use elide::entity::{Entity, LabelRef};
use elide::modality::text::{Text, TextLocation};
use elide::primitive::Confidence;
use elide_review::{Edit, EditSet};
use uuid::Uuid;

fn entity(label: &str, at: (usize, usize)) -> Entity<Text> {
    let location = TextLocation::new(at.0, at.1);
    let event = AuditEvent::pattern(
        "probe",
        Confidence::MAX,
        location.clone(),
        elide::entity::audit::PatternEvent::default(),
    );
    Entity::new(
        LabelRef::new(label),
        location,
        Confidence::MAX,
        AuditLog::new(event),
    )
}

fn report_with_one() -> (Report, Uuid) {
    let e = entity("email_address", (0, 5));
    let id = e.id;
    (Report::new().insert_body::<Text>(vec![e]), id)
}

#[test]
fn edits_deserialize_from_a_request_body() {
    // The shape an HTTP layer receives, with no engine in scope.
    let body = r#"{
        "text": [
            {"op": "suppress", "id": "01958ccd-0000-7000-8000-000000000001",
             "reason": "false positive", "actor": "alice"},
            {"op": "add", "label": "phone_number",
             "location": {"range": {"start": 10, "end": 22}}}
        ]
    }"#;
    let edits: EditSet = serde_json::from_str(body).expect("edits deserialize");
    assert_eq!(edits.len(), 2);
    edits.validate().expect("no contradictions");
}

#[test]
fn contradictions_are_caught_before_anything_is_applied() {
    let id = Uuid::from_u128(1);
    let mut edits = EditSet::default();
    edits.text.push(Edit::Suppress {
        id,
        reason: None,
        actor: None,
    });
    edits.text.push(Edit::Redact {
        id,
        policy_id: Uuid::from_u128(9),
        action: elide_governance::redaction::TextRedaction::Erase,
        reason: None,
        actor: None,
    });

    let err = edits
        .validate()
        .expect_err("suppress and redact contradict");
    assert!(err.to_string().contains(&id.to_string()), "{err}");
}

#[test]
fn apply_lands_add_retag_and_suppress_on_the_report() {
    let (mut report, id) = report_with_one();
    let mut edits = EditSet::default();

    edits.text.push(Edit::Add {
        label: LabelRef::new("phone_number"),
        location: TextLocation::new(10, 22),
        reason: None,
        actor: Some("alice".into()),
    });
    edits.text.push(Edit::Retag {
        id,
        label: Some(LabelRef::new("person_name")),
        location: None,
        reason: None,
        actor: None,
    });

    edits.validate().expect("composable");
    edits.apply(&mut report);

    let entities = report.entities::<Text>().expect("text body");
    assert_eq!(entities.len(), 2, "the added entity is on the report");

    let retagged = entities.iter().find(|e| e.id == id).expect("original");
    assert_eq!(retagged.label.as_str(), "person_name", "retag landed");

    let added = entities.iter().find(|e| e.id != id).expect("added");
    assert!(
        added
            .audit
            .events()
            .iter()
            .any(|e| matches!(e.kind, AuditKind::Manual(_))),
        "an added entity carries human provenance",
    );

    // Applied edits are history; nothing is left pending.
    assert!(edits.text.is_empty(), "add and retag are consumed");
}

#[test]
fn suppress_marks_the_entity_and_a_redact_lifts_it() {
    let (mut report, id) = report_with_one();

    let mut edits = EditSet::default();
    edits.text.push(Edit::Suppress {
        id,
        reason: Some("fp".into()),
        actor: None,
    });
    edits.apply(&mut report);

    let e = report
        .entities::<Text>()
        .unwrap()
        .iter()
        .find(|e| e.id == id)
        .unwrap();
    assert!(e.is_suppressed(), "suppression stamped on the trail");

    // A later pass reverses it — the round trip a reviewer makes.
    let mut edits = EditSet::default();
    edits.text.push(Edit::Redact {
        id,
        policy_id: Uuid::from_u128(9),
        action: elide_governance::redaction::TextRedaction::Erase,
        reason: None,
        actor: None,
    });
    edits.apply(&mut report);

    let e = report
        .entities::<Text>()
        .unwrap()
        .iter()
        .find(|e| e.id == id)
        .unwrap();
    assert!(!e.is_suppressed(), "the redact lifted it");
    assert_eq!(
        edits.text.len(),
        1,
        "the redact stays pending for the anonymizer"
    );
}
