//! Everything a consumer can do with this crate and `elide` alone:
//! build a report, deserialize edits from a request body, validate
//! them, and land them on the report.
//!
//! No engine, no provider, no pipeline — if this file compiles, the
//! crate stands on its own.

use elide::Report;
use elide::entity::audit::{Attribution, AuditEvent, AuditKind, AuditLog, ManualIntent};
use elide::entity::{Entity, LabelRef};
use elide::modality::text::{Text, TextLocation};
use elide::primitive::Confidence;
use elide_review::{Add, Edit, EditSet, Redact, Retag, Reviewer, Suppress};
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
    edits.text.push(Edit::Suppress(Suppress {
        id,
        by: Reviewer {
            reason: None,
            actor: None,
        },
    }));
    edits.text.push(Edit::Redact(Redact {
        id,
        policy_id: Uuid::from_u128(9),
        action: elide_governance::redaction::TextRedaction::Erase,
        by: Reviewer {
            reason: None,
            actor: None,
        },
    }));

    let err = edits
        .validate()
        .expect_err("suppress and redact contradict");
    assert!(err.to_string().contains(&id.to_string()), "{err}");
}

#[test]
fn apply_lands_add_retag_and_suppress_on_the_report() {
    let (mut report, id) = report_with_one();
    let mut edits = EditSet::default();

    edits.text.push(Edit::Add(Add {
        label: LabelRef::new("phone_number"),
        location: TextLocation::new(10, 22),
        by: Reviewer {
            reason: None,
            actor: Some("alice".into()),
        },
    }));
    edits.text.push(Edit::Retag(Retag {
        id,
        label: Some(LabelRef::new("person_name")),
        location: None,
        by: Reviewer {
            reason: None,
            actor: None,
        },
    }));

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
    edits.text.push(Edit::Suppress(Suppress {
        id,
        by: Reviewer {
            reason: Some("fp".into()),
            actor: None,
        },
    }));
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
    edits.text.push(Edit::Redact(Redact {
        id,
        policy_id: Uuid::from_u128(9),
        action: elide_governance::redaction::TextRedaction::Erase,
        by: Reviewer {
            reason: None,
            actor: None,
        },
    }));
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

#[test]
fn a_third_retag_conflicts_with_the_first() {
    // label, then location, then label again. The middle edit
    // merges with both neighbours, so comparing only against the
    // most recent would let two labels through and silently apply
    // the last.
    let id = Uuid::from_u128(1);
    let retag = |label: Option<&str>, location: Option<TextLocation>| {
        Edit::Retag(Retag {
            id,
            label: label.map(LabelRef::new),
            location,
            by: Reviewer {
                reason: None,
                actor: None,
            },
        })
    };

    let mut edits = EditSet::default();
    edits.text.push(retag(Some("a"), None));
    edits.text.push(retag(None, Some(TextLocation::new(0, 4))));
    edits.text.push(retag(Some("b"), None));

    let err = edits
        .validate()
        .expect_err("two labels for one entity contradict, however they are interleaved");
    assert!(err.to_string().contains(&id.to_string()), "{err}");
}

#[test]
fn an_added_entity_keeps_its_reason_and_actor() {
    // The edit is consumed once applied, so if the trail does not
    // carry them the audit can no longer say who added the entity.
    let mut report = Report::new().insert_body::<Text>(Vec::new());
    let mut edits = EditSet::default();
    edits.text.push(Edit::Add(Add {
        label: LabelRef::new("phone_number"),
        location: TextLocation::new(10, 22),
        by: Reviewer {
            reason: Some("recognizer missed it".into()),
            actor: Some("alice".into()),
        },
    }));
    edits.apply(&mut report);

    let added = &report.entities::<Text>().expect("text body")[0];
    let event = added
        .audit
        .events()
        .iter()
        .find(|e| matches!(e.kind, AuditKind::Manual(_)))
        .expect("an added entity carries a Manual event");
    assert_eq!(
        event.source.as_str(),
        "alice",
        "the reviewer is the event's source",
    );
    let AuditKind::Manual(manual) = &event.kind else {
        unreachable!("matched above")
    };
    let Some(Attribution::Freeform(freeform)) = &manual.attribution else {
        panic!("the rationale rides on a freeform attribution");
    };
    assert_eq!(freeform.name.as_str(), "recognizer missed it");
}

#[test]
fn retagging_does_not_unsuppress() {
    // A retag records `ManualIntent::Amend`, which `is_suppressed`
    // skips: a correction says nothing about whether the entity is
    // redacted, so it must not revive a suppressed one.
    let (mut report, id) = report_with_one();

    let mut edits = EditSet::default();
    edits.text.push(Edit::Suppress(Suppress {
        id,
        by: Reviewer {
            reason: None,
            actor: None,
        },
    }));
    edits.apply(&mut report);

    let mut edits = EditSet::default();
    edits.text.push(Edit::Retag(Retag {
        id,
        label: Some(LabelRef::new("person_name")),
        location: None,
        by: Reviewer {
            reason: Some("wrong label".into()),
            actor: Some("bob".into()),
        },
    }));
    edits.apply(&mut report);

    let e = report
        .entities::<Text>()
        .unwrap()
        .iter()
        .find(|e| e.id == id)
        .unwrap();
    assert!(
        e.is_suppressed(),
        "a correction must not lift a suppression"
    );
    assert_eq!(
        e.label.as_str(),
        "person_name",
        "but the retag still applied"
    );
}

#[test]
fn an_edit_that_finds_no_entity_stays_pending() {
    // Each modality gets its own pass over the same bucket, and an
    // id may simply be stale. Dropping an edit that changed nothing
    // would make a reviewer's decision vanish with no error and no
    // record of it.
    let mut report = Report::new().insert_body::<Text>(Vec::new());
    let mut edits = EditSet::default();
    edits.text.push(Edit::Suppress(Suppress {
        id: Uuid::from_u128(999),
        by: Reviewer {
            reason: Some("false positive".into()),
            actor: Some("alice".into()),
        },
    }));

    edits.apply(&mut report);

    assert_eq!(
        edits.text.len(),
        1,
        "an edit naming an entity the report does not hold is not applied, so it is not history",
    );
}

#[test]
fn an_applied_edit_stops_being_pending() {
    // The counterpart: what *did* land is history, because the
    // entity's own trail now carries it.
    let (mut report, id) = report_with_one();
    let mut edits = EditSet::default();
    edits.text.push(Edit::Suppress(Suppress {
        id,
        by: Reviewer::default(),
    }));

    edits.apply(&mut report);

    assert!(edits.text.is_empty(), "the applied suppression is consumed");
}

#[test]
fn a_retag_records_who_corrected_it() {
    // `ManualIntent::Amend` is what lets a correction carry the
    // reviewer's rationale onto the trail — the edit itself is
    // consumed once applied, so without this the audit could not
    // say who changed the entity.
    let (mut report, id) = report_with_one();
    let mut edits = EditSet::default();
    edits.text.push(Edit::Retag(Retag {
        id,
        label: Some(LabelRef::new("person_name")),
        location: None,
        by: Reviewer {
            reason: Some("recognizer mislabelled it".into()),
            actor: Some("bob".into()),
        },
    }));

    edits.apply(&mut report);

    let entity = &report.entities::<Text>().expect("text body")[0];
    let event = entity
        .audit
        .events()
        .iter()
        .find(|e| matches!(&e.kind, AuditKind::Manual(m) if m.intent == ManualIntent::Amend))
        .expect("a retag records an Amend event");
    assert_eq!(event.source.as_str(), "bob");
    let AuditKind::Manual(manual) = &event.kind else {
        unreachable!("matched above")
    };
    let Some(Attribution::Freeform(freeform)) = &manual.attribution else {
        panic!("the rationale rides on a freeform attribution");
    };
    assert_eq!(freeform.name.as_str(), "recognizer mislabelled it");
}
