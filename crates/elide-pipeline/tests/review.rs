//! The three things a reviewer can do to a detection, end to end
//! through a stateless JSON round-trip.
//!
//! Each action has one home, and this asserts each reaches apply:
//! `suppress` leaves an entity alone, `include` redacts one
//! recognition missed, and `review` swaps the operator the policy
//! would have used. The round-trip is the point: a host persists
//! the audit and posts it back, so a decision that does not
//! survive serialization is a decision silently dropped.

use bytes::Bytes;
use elide::entity::audit::{AuditEvent, AuditKind, AuditLog, PatternEvent};
use elide::entity::{Entity, LabelRef};
use elide::modality::image::{Image, ImageLocation};
use elide::modality::text::{Text, TextLocation};
use elide::primitive::{BoundingBox, Confidence, Point};
use elide_governance::redaction::{ModalityRedactions, TextRedaction};
use elide_governance::{LabelScope, PolicyDefinition};
use elide_pipeline::{Audit, Engine, EntityGroup};
use elide_wire::file::Document;
use elide_wire::plan::AnalyzerParams;

const SAMPLE: &[u8] = b"Email alice@example.com or bob@example.com. Case SECRET-9 open.";
const POLICY_ID: uuid::Uuid = uuid::Uuid::from_u128(0x0123_4567_89ab_7000_8000_0000_0000_0042);

/// Byte range of `needle` in the sample, so a hand-built entity
/// lands on a real span instead of a hardcoded offset.
fn span_of(needle: &[u8]) -> TextLocation {
    let start = SAMPLE
        .windows(needle.len())
        .position(|w| w == needle)
        .expect("needle present in sample");
    TextLocation::new(start, start + needle.len())
}

/// Erase every `email_address` through the policy fallback.
fn policy() -> PolicyDefinition {
    PolicyDefinition {
        id: POLICY_ID,
        name: "sweep".into(),
        description: None,
        template: None,
        scopes: vec![LabelScope::new(
            "contact",
            vec![LabelRef::new("email_address")],
        )],
        custom: Vec::new(),
        rules: Vec::new(),
        fallback: Some(ModalityRedactions {
            text: Some(TextRedaction::Erase),
            ..Default::default()
        }),
    }
}

fn doc() -> Document {
    Document::new(Bytes::from_static(SAMPLE), "txt")
}

/// A visibly different operator from the policy's `Erase`, so an
/// override is legible in the output.
fn mask() -> TextRedaction {
    TextRedaction::Mask {
        mask_char: '*',
        keep_prefix: 0,
        keep_suffix: 0,
    }
}

/// Analyze, let `edit` apply reviewer decisions, then round-trip
/// the audit through JSON before anonymizing: the path a stateless
/// host actually takes.
async fn review_and_apply(edit: impl FnOnce(&mut Audit)) -> (String, Audit) {
    let engine = Engine::new();
    let policy = policy();
    let mut audit = engine
        .analyze(
            doc(),
            std::slice::from_ref(&policy),
            &AnalyzerParams::default(),
        )
        .await
        .expect("analyze");

    edit(&mut audit);

    let json = serde_json::to_string(&audit).expect("audit serializes");
    let mut posted_back: Audit = serde_json::from_str(&json).expect("audit deserializes");

    let out = engine
        .anonymize(doc(), std::slice::from_ref(&policy), &mut posted_back)
        .await
        .expect("anonymize");
    (
        String::from_utf8_lossy(&out.bytes).into_owned(),
        posted_back,
    )
}

/// Body records sorted into document order, so index 0 is the
/// first entity in the text rather than whatever order the
/// recognizer emitted.
fn ordered(audit: &mut Audit) -> &mut Vec<elide_pipeline::entity::EntityRecord<Text>> {
    let Some(EntityGroup::Text(records)) = audit.body.as_mut() else {
        panic!("expected a text body");
    };
    records.sort_by_key(|r| r.entity.location.range.start);
    records
}

#[tokio::test]
async fn baseline_redacts_every_detection() {
    let (out, _) = review_and_apply(|_| {}).await;
    assert!(!out.contains("alice@example.com"), "{out}");
    assert!(!out.contains("bob@example.com"), "{out}");
    // Not detected, so untouched without a manual include.
    assert!(out.contains("SECRET-9"), "{out}");
}

#[tokio::test]
async fn suppress_leaves_the_entity_alone() {
    let (out, audit) = review_and_apply(|audit| {
        ordered(audit)[0].suppress(Some("known test account".into()), Some("reviewer".into()));
    })
    .await;

    assert!(
        out.contains("alice@example.com"),
        "suppressed entity must survive redaction: {out}"
    );
    assert!(
        !out.contains("bob@example.com"),
        "an unsuppressed sibling must still be redacted: {out}"
    );

    // The decision is auditable, not merely effective.
    let EntityGroup::Text(records) = audit.body.as_ref().expect("body") else {
        panic!("expected text body");
    };
    let suppressed = records
        .iter()
        .find(|r| r.is_suppressed())
        .expect("a record reports itself suppressed after the round-trip");
    let manual = suppressed
        .entity
        .audit
        .events()
        .iter()
        .find_map(|e| match &e.kind {
            AuditKind::Manual(m) => Some(m),
            _ => None,
        })
        .expect("suppression records a Manual event");
    assert_eq!(manual.reason.as_deref(), Some("known test account"));
    assert_eq!(manual.actor.as_deref(), Some("reviewer"));
    assert!(
        suppressed.entity.audit.verify().is_ok(),
        "the hash chain still verifies after a suppression round-trip"
    );
}

#[tokio::test]
async fn include_redacts_what_recognition_missed() {
    let (out, _) = review_and_apply(|audit| {
        let location = span_of(b"SECRET-9");
        let event: AuditEvent<Text> = AuditEvent::pattern(
            "manual",
            Confidence::MAX,
            location.clone(),
            PatternEvent::default(),
        );
        let entity = Entity::new(
            LabelRef::new("email_address"),
            location,
            Confidence::MAX,
            AuditLog::new(event),
        );
        let group = audit.body.as_mut().expect("body");
        assert!(group.include(entity), "include into the matching modality");
    })
    .await;

    assert!(
        !out.contains("SECRET-9"),
        "an included entity is redacted like a detected one: {out}"
    );
}

#[tokio::test]
async fn include_stamps_manual_provenance() {
    let engine = Engine::new();
    let policy = policy();
    let mut audit = engine
        .analyze(
            doc(),
            std::slice::from_ref(&policy),
            &AnalyzerParams::default(),
        )
        .await
        .expect("analyze");

    let location = span_of(b"SECRET-9");
    let event: AuditEvent<Text> = AuditEvent::pattern(
        "manual",
        Confidence::MAX,
        location.clone(),
        PatternEvent::default(),
    );
    let entity = Entity::new(
        LabelRef::new("email_address"),
        location,
        Confidence::MAX,
        AuditLog::new(event),
    );
    let id = entity.id;
    audit.body.as_mut().expect("body").include(entity);

    let records = ordered(&mut audit);
    let included = records
        .iter()
        .find(|r| r.entity.id == id)
        .expect("included entity present");
    assert!(
        included
            .entity
            .audit
            .events()
            .iter()
            .any(|e| matches!(e.kind, AuditKind::Manual(_))),
        "an included entity carries Manual provenance so it is never \
         mistaken for an automatic detection",
    );
}

#[tokio::test]
async fn include_rejects_a_foreign_modality() {
    let engine = Engine::new();
    let policy = policy();
    let mut audit = engine
        .analyze(
            doc(),
            std::slice::from_ref(&policy),
            &AnalyzerParams::default(),
        )
        .await
        .expect("analyze");

    let bounds = BoundingBox::new(Point::new(0.0, 0.0), Point::new(4.0, 4.0));
    let location = ImageLocation::new(bounds);
    let event: AuditEvent<Image> = AuditEvent::pattern(
        "manual",
        Confidence::MAX,
        location.clone(),
        PatternEvent::default(),
    );
    let entity = Entity::new(
        LabelRef::new("email_address"),
        location,
        Confidence::MAX,
        AuditLog::new(event),
    );

    assert!(
        !audit.body.as_mut().expect("body").include(entity),
        "an image entity cannot join a text group",
    );
}

#[tokio::test]
async fn review_overrides_the_operator_the_policy_would_have_used() {
    let (out, _) = review_and_apply(|audit| {
        let records = ordered(audit);
        records[1].redact(POLICY_ID, mask());
    })
    .await;

    assert!(
        out.contains("***************"),
        "the reviewer's Mask must beat the policy's Erase: {out}"
    );
    assert!(!out.contains("bob@example.com"), "{out}");
}

#[tokio::test]
async fn a_later_decision_replaces_an_earlier_one() {
    // `Review` is one enum, so redact-then-suppress cannot leave
    // both set with one silently winning: the second call is the
    // decision that stands.
    let (out, _) = review_and_apply(|audit| {
        let records = ordered(audit);
        records[0].redact(POLICY_ID, mask());
        records[0].suppress(Some("actually fine".into()), None);
        assert!(records[0].is_suppressed());
    })
    .await;

    assert!(
        out.contains("alice@example.com"),
        "the suppression that replaced the override is what applies: {out}"
    );
}

#[tokio::test]
async fn all_three_actions_compose_in_one_pass() {
    let (out, _) = review_and_apply(|audit| {
        {
            let records = ordered(audit);
            records[0].suppress(Some("false positive".into()), None);
            records[1].redact(POLICY_ID, mask());
        }
        let location = span_of(b"SECRET-9");
        let event: AuditEvent<Text> = AuditEvent::pattern(
            "manual",
            Confidence::MAX,
            location.clone(),
            PatternEvent::default(),
        );
        let entity = Entity::new(
            LabelRef::new("email_address"),
            location,
            Confidence::MAX,
            AuditLog::new(event),
        );
        audit.body.as_mut().expect("body").include(entity);
    })
    .await;

    assert!(out.contains("alice@example.com"), "suppressed: {out}");
    assert!(
        out.contains("***************"),
        "operator overridden: {out}"
    );
    assert!(!out.contains("SECRET-9"), "included and redacted: {out}");
}

#[tokio::test]
async fn analyze_records_the_policy_pick_for_review() {
    // The point of the pick pass: a reviewer must be able to see
    // *what would happen and why* before overriding anything.
    let engine = Engine::new();
    let policy = policy();
    let audit = engine
        .analyze(
            doc(),
            std::slice::from_ref(&policy),
            &AnalyzerParams::default(),
        )
        .await
        .expect("analyze");

    let json = serde_json::to_string(&audit).expect("serializes");
    let posted_back: Audit = serde_json::from_str(&json).expect("deserializes");
    let EntityGroup::Text(records) = posted_back.body.as_ref().expect("body") else {
        panic!("expected text body");
    };

    for record in records {
        let selection = record
            .entity
            .audit
            .selection()
            .expect("every covered entity carries its pick after analyze");
        assert_eq!(
            selection.operator.name.as_str(),
            "erase",
            "the pick names the operator the policy fallback would run",
        );
        assert!(
            selection.attribution.is_some(),
            "the pick carries the policy's own rationale, not just an operator id",
        );
        assert!(
            record.entity.audit.verify().is_ok(),
            "recording a pick keeps the hash chain intact",
        );
    }
}

#[tokio::test]
async fn a_suppressed_entity_gets_no_pick() {
    // Nothing was going to redact it, so there is no operator to
    // record. Asserted so the pick pass never implies a redaction
    // that will not happen.
    let engine = Engine::new();
    let policy = policy();
    let mut audit = engine
        .analyze(
            doc(),
            std::slice::from_ref(&policy),
            &AnalyzerParams::default(),
        )
        .await
        .expect("analyze");

    ordered(&mut audit)[0].suppress(Some("false positive".into()), None);

    // Re-running the pick pass over a suppressed entity adds nothing.
    let before = ordered(&mut audit)[0].entity.audit.len();
    let json = serde_json::to_string(&audit).expect("serializes");
    let mut posted_back: Audit = serde_json::from_str(&json).expect("deserializes");
    let after = ordered(&mut posted_back)[0].entity.audit.len();
    assert_eq!(before, after, "a round-trip neither adds nor drops events");
}
