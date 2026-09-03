//! Everything a consumer can do with this crate and `elide` alone:
//! build a report, deserialize edits from a request body, validate
//! them, and land them on the report.
//!
//! No engine, no provider, no pipeline — if this file compiles, the
//! crate stands on its own.

use elide::entity::audit::{Attribution, AuditEvent, AuditKind, AuditLog, ManualIntent};
use elide::entity::{Entity, LabelRef};
use elide::modality::image::{Image, ImageLocation};
use elide::modality::text::{Text, TextLocation};
use elide::primitive::{BoundingBox, Confidence, Point};
use elide::{PartId, Report};
use elide_review::{Add, Edit, EditError, EditSet, Retag, Reviewer, Suppress};
use uuid::Uuid;

fn entity(label: &str, at: (usize, usize)) -> Entity<Text> {
    let location = TextLocation::new(at.0, at.1);
    let event = AuditEvent::pattern(
        "probe",
        Confidence::MAX,
        location.clone(),
        elide::entity::audit::PatternEvent::default(),
    );
    Entity::new(LabelRef::new(label), location, AuditLog::new(event))
}

/// The one document these reports hold. Since elide unified the
/// body into the part tree, a document is a named depth-1 part and
/// anything it embeds is a child of it.
const DOCUMENT: &str = "report.docx";

fn document() -> PartId {
    PartId::new(DOCUMENT)
}

fn report_with_one() -> (Report, Uuid) {
    let e = entity("email_address", (0, 5));
    let id = e.id;
    (Report::new().insert_part::<Text>(document(), vec![e]), id)
}

#[test]
fn edits_deserialize_from_a_request_body() {
    // The shape an HTTP layer receives, with no engine in scope.
    let body = r#"{
        "text": [
            {"op": "suppress", "id": "01958ccd-0000-7000-8000-000000000001",
             "reason": "false positive", "actor": "alice"},
            {"op": "add", "label": "phone_number",
             "location": {
                 "coord": {
                     "kind": "source",
                     "source": [{"range": {"start": 200, "end": 215},
                                 "part": "word/document.xml"}]
                 }
             }}
        ]
    }"#;
    let edits: EditSet = serde_json::from_str(body).expect("edits deserialize");
    assert_eq!(edits.len(), 2);

    // A reviewer selecting rendered text in a container has raw
    // file bytes, not a decoded offset, so the location is
    // source-only: there is no range to give, and elide models that
    // as its own coordinate kind rather than a zero-length span.
    // The engine reverse-resolves `source`, and the part rides
    // along inside it. The add names no part of its own: a
    // container's text belongs to the document.
    let Some(Edit::Add(add)) = edits.text.get(1) else {
        panic!("the second edit is the add");
    };
    assert_eq!(add.part, None, "text goes to the document itself");
    assert_eq!(
        add.location.range(),
        None,
        "a reviewer selection has no decoded range to report",
    );
    assert_eq!(
        add.location
            .source()
            .first()
            .and_then(|s| s.part.as_deref()),
        Some("word/document.xml"),
        "and the part is carried by the source reference",
    );

    // Validating needs the report the edits target — an id is only
    // meaningful against one — so a handler parses here and
    // validates once it has the audit.
}

#[test]
fn contradictions_are_caught_before_anything_is_applied() {
    // Two retags setting the same field are two answers to one
    // question. Disjoint fields merge, and a repeated suppress
    // dedupes, so this is the pair that has to be rejected.
    let (report, id) = report_with_one();
    let retag = |label: &str| {
        Edit::Retag(Retag {
            id,
            label: Some(LabelRef::new(label)),
            location: None,
            by: Reviewer::default(),
        })
    };
    let mut edits = EditSet::default();
    edits.text.push(retag("person_name"));
    edits.text.push(retag("email_address"));

    let err = edits
        .validate(&report)
        .expect_err("two labels for one entity contradict");
    assert!(err.to_string().contains(&id.to_string()), "{err}");
}

#[test]
fn apply_lands_add_retag_and_suppress_on_the_report() {
    let (mut report, id) = report_with_one();
    let mut edits = EditSet::default();

    edits.text.push(Edit::Add(Add {
        label: LabelRef::new("phone_number"),
        location: TextLocation::new(10, 22),
        part: None,
        by: Reviewer::actor("alice"),
    }));
    edits.text.push(Edit::Retag(Retag {
        id,
        label: Some(LabelRef::new("person_name")),
        location: None,
        by: Reviewer::default(),
    }));

    edits.apply(&mut report).expect("composable");

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

    // The caller keeps its edits either way; see
    // `applying_leaves_the_caller_s_edits_alone`.
    assert_eq!(edits.text.len(), 2, "the caller's edits are untouched");
}

#[test]
fn suppress_stamps_the_entity() {
    let (mut report, id) = report_with_one();

    let mut edits = EditSet::default();
    edits.text.push(Edit::Suppress(Suppress {
        id,
        by: Reviewer::reason("fp"),
    }));
    edits
        .apply(&mut report)
        .expect("the edits apply to this report");

    let e = report
        .entities::<Text>()
        .unwrap()
        .iter()
        .find(|e| e.id == id)
        .unwrap();
    assert!(e.is_suppressed(), "suppression stamped on the trail");
}

#[test]
fn a_third_retag_conflicts_with_the_first() {
    // label, then location, then label again. The middle edit
    // merges with both neighbours, so comparing only against the
    // most recent would let two labels through and silently apply
    // the last.
    let (report, id) = report_with_one();
    let retag = |label: Option<&str>, location: Option<TextLocation>| {
        Edit::Retag(Retag {
            id,
            label: label.map(LabelRef::new),
            location,
            by: Reviewer::default(),
        })
    };

    let mut edits = EditSet::default();
    edits.text.push(retag(Some("a"), None));
    edits.text.push(retag(None, Some(TextLocation::new(0, 4))));
    edits.text.push(retag(Some("b"), None));

    let err = edits
        .validate(&report)
        .expect_err("two labels for one entity contradict, however they are interleaved");
    assert!(err.to_string().contains(&id.to_string()), "{err}");
}

#[test]
fn an_added_entity_keeps_its_reason_and_actor() {
    // The edit is consumed once applied, so if the trail does not
    // carry them the audit can no longer say who added the entity.
    let mut report = Report::new().insert_part::<Text>(document(), Vec::new());
    let mut edits = EditSet::default();
    edits.text.push(Edit::Add(Add {
        label: LabelRef::new("phone_number"),
        location: TextLocation::new(10, 22),
        part: None,
        by: Reviewer::reason("recognizer missed it").with_actor("alice"),
    }));
    edits
        .apply(&mut report)
        .expect("the edits apply to this report");

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
        by: Reviewer::default(),
    }));
    edits
        .apply(&mut report)
        .expect("the edits apply to this report");

    let mut edits = EditSet::default();
    edits.text.push(Edit::Retag(Retag {
        id,
        label: Some(LabelRef::new("person_name")),
        location: None,
        by: Reviewer::reason("wrong label").with_actor("bob"),
    }));
    edits
        .apply(&mut report)
        .expect("the edits apply to this report");

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
fn applying_leaves_the_caller_s_edits_alone() {
    // `apply` takes `&self`: an edit set is the caller's own input,
    // not state this crate keeps. A server holds the set it parsed
    // from the request and can still report on it afterwards.
    let (mut report, id) = report_with_one();
    let mut edits = EditSet::default();
    edits.text.push(Edit::Suppress(Suppress {
        id,
        by: Reviewer::default(),
    }));
    edits.text.push(Edit::Suppress(Suppress {
        // A second decision on the same channel that merges with
        // the first rather than contradicting it.
        id,
        by: Reviewer::default(),
    }));

    edits
        .apply(&mut report)
        .expect("the edits apply to this report");

    assert_eq!(edits.text.len(), 2, "both edits survive the apply");
    let stamped = report
        .entities::<Text>()
        .expect("text body")
        .iter()
        .find(|e| e.id == id)
        .expect("the real entity")
        .is_suppressed();
    assert!(stamped, "the edit that found its target still applied");
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
        by: Reviewer::reason("recognizer mislabelled it").with_actor("bob"),
    }));

    edits
        .apply(&mut report)
        .expect("the edits apply to this report");

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

#[test]
fn an_edit_naming_no_entity_in_the_report_is_rejected() {
    // `apply` skips a target it cannot find, without a word. A
    // reviewer would be told their suppression took effect while
    // the document still carries the entity, so the set has to be
    // rejected before it is applied.
    let (report, _) = report_with_one();
    let stale = Uuid::from_u128(999);

    let mut edits = EditSet::default();
    edits.edit(Edit::<Text>::Suppress(Suppress {
        id: stale,
        by: Reviewer::default(),
    }));

    let err = edits
        .validate(&report)
        .expect_err("a stale id is not applicable to this report");
    assert_eq!(err.entity_id(), Some(stale));
    assert!(
        matches!(err, EditError::UnknownTarget { .. }),
        "the reason is the missing target, not a contradiction: {err}",
    );
}

#[test]
fn an_edit_filed_under_the_wrong_modality_is_rejected() {
    // Each modality is applied in its own pass, so a text entity's
    // id in the image bucket finds nothing and vanishes silently.
    // The report is searched under the bucket's modality, so this
    // reads as an unknown target — which it is, for that modality.
    let (report, id) = report_with_one();

    let mut edits = EditSet::default();
    edits.image.push(Edit::Suppress(Suppress {
        id,
        by: Reviewer::default(),
    }));

    let err = edits
        .validate(&report)
        .expect_err("a text entity is not an image entity");
    assert!(
        matches!(
            err,
            EditError::UnknownTarget {
                modality: "image",
                ..
            }
        ),
        "reported against the bucket it was filed under: {err}",
    );
}

#[test]
fn an_image_add_lands_in_the_part_it_names() {
    // The case this field exists for. A container's embedded media
    // is a report group of its own, so an image entity lives under
    // its part — a reviewer boxing a face in `image1.png` has
    // nowhere to put it otherwise.
    //
    // Text does not need this: a DOCX's `word/document.xml` text
    // belongs to the document part itself, and where a span came
    // from is carried by `TextLocation::source`.
    let part = document().child("word/media/image1.png");
    let mut report = Report::new()
        .insert_part::<Text>(document(), Vec::new())
        .insert_part::<Image>(part.clone(), Vec::new());

    let mut edits = EditSet::default();
    edits.edit(Edit::Add(Add::<Image> {
        label: LabelRef::new("person_name"),
        location: ImageLocation::new(BoundingBox::new(Point::new(0.0, 0.0), Point::new(4.0, 4.0))),
        part: Some(vec![
            DOCUMENT.to_owned(),
            "word/media/image1.png".to_owned(),
        ]),
        by: Reviewer::default(),
    }));

    edits.apply(&mut report).expect("the part exists");

    assert_eq!(
        report
            .part_entities::<Image>(&part)
            .expect("the part carries entities")
            .len(),
        1,
        "the addition landed in the part",
    );
}

#[test]
fn an_add_naming_an_absent_part_is_rejected() {
    // `include_part` returns `false` for a part the report does not
    // carry, so without this the addition would vanish and the
    // reviewer would be told it landed.
    let report = Report::new().insert_part::<Text>(document(), Vec::new());

    let mut edits = EditSet::default();
    edits.edit(Edit::Add(Add::<Text> {
        label: LabelRef::new("email_address"),
        location: TextLocation::new(0, 4),
        part: Some(vec![DOCUMENT.to_owned(), "word/nonexistent.xml".to_owned()]),
        by: Reviewer::default(),
    }));

    let err = edits
        .validate(&report)
        .expect_err("the part is not in this report");
    assert!(
        matches!(&err, EditError::UnknownPart { part, .. } if part.last().map(String::as_str) == Some("word/nonexistent.xml")),
        "the error names the missing part: {err}",
    );
    assert_eq!(err.entity_id(), None, "an add names no entity");
}
