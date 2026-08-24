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
use elide_pipeline::entity::Review;
use elide_pipeline::file::Document;
use elide_pipeline::plan::AnalyzerParams;
use elide_pipeline::{Audit, Engine, ProviderConfig, RequestContext};
use uuid::Uuid;

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
    let engine = Engine::new(ProviderConfig::default().build());
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
    let mut posted_back = round_trip(&engine, &json);

    let out = engine
        .anonymize(
            doc(),
            std::slice::from_ref(&policy),
            &mut posted_back,
            &RequestContext::new(),
        )
        .await
        .expect("anonymize");
    (
        String::from_utf8_lossy(&out.bytes).into_owned(),
        posted_back,
    )
}

/// Read an audit back the way a host does: through the engine,
/// which holds the modality registry a serialized report needs to
/// be rebuilt.
fn round_trip(engine: &Engine, json: &str) -> Audit {
    let mut de = serde_json::Deserializer::from_str(json);
    engine
        .deserialize_audit(&mut de)
        .expect("audit deserializes")
}

/// A detection a reviewer supplies by hand, on `location`.
///
/// `Report::include` stamps the `Manual` provenance as it lands, so
/// the trail this carries is only the seed every entity needs.
fn manual_entity(location: TextLocation) -> Entity<Text> {
    let event: AuditEvent<Text> = AuditEvent::pattern(
        "manual",
        Confidence::MAX,
        location.clone(),
        PatternEvent::default(),
    );
    Entity::new(
        LabelRef::new("email_address"),
        location,
        Confidence::MAX,
        AuditLog::new(event),
    )
}

/// Body entity ids in document order, so index 0 is the first
/// entity in the text rather than whatever order the recognizer
/// emitted. Decisions key by id, so this is what a caller indexes.
fn ordered(audit: &Audit) -> Vec<Uuid> {
    let mut entities: Vec<&Entity<Text>> = audit
        .report
        .entities::<Text>()
        .expect("a text body")
        .iter()
        .collect();
    entities.sort_by_key(|e| e.location.range.start);
    entities.iter().map(|e| e.id).collect()
}

/// The body entity `id`, for asserting on its trail.
fn entity(audit: &Audit, id: Uuid) -> &Entity<Text> {
    audit
        .report
        .entities::<Text>()
        .expect("a text body")
        .iter()
        .find(|e| e.id == id)
        .expect("entity present")
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
        let id = ordered(audit)[0];
        audit.review::<Text>(
            id,
            Review::Suppress {
                reason: Some("known test account".into()),
                actor: Some("reviewer".into()),
            },
        );
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
    let suppressed = audit
        .report
        .entities::<Text>()
        .expect("a text body")
        .iter()
        .find(|e| e.is_suppressed())
        .expect("an entity reports itself suppressed after the round-trip");
    let manual = suppressed
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
        suppressed.audit.verify().is_ok(),
        "the hash chain still verifies after a suppression round-trip"
    );
}

#[tokio::test]
async fn include_redacts_what_recognition_missed() {
    let (out, _) = review_and_apply(|audit| {
        let added = audit
            .report
            .include::<Text>(manual_entity(span_of(b"SECRET-9")));
        assert!(added, "include into the matching modality");
    })
    .await;

    assert!(
        !out.contains("SECRET-9"),
        "an included entity is redacted like a detected one: {out}"
    );
}

#[tokio::test]
async fn include_stamps_manual_provenance() {
    let engine = Engine::new(ProviderConfig::default().build());
    let policy = policy();
    let mut audit = engine
        .analyze(
            doc(),
            std::slice::from_ref(&policy),
            &AnalyzerParams::default(),
        )
        .await
        .expect("analyze");

    let added = manual_entity(span_of(b"SECRET-9"));
    let id = added.id;
    audit.report.include::<Text>(added);

    assert!(
        entity(&audit, id)
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
    let engine = Engine::new(ProviderConfig::default().build());
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
    let foreign = Entity::new(
        LabelRef::new("email_address"),
        location,
        Confidence::MAX,
        AuditLog::new(event),
    );

    assert!(
        !audit.report.include::<Image>(foreign),
        "an image entity cannot join a text body",
    );
}

#[tokio::test]
async fn review_overrides_the_operator_the_policy_would_have_used() {
    let (out, _) = review_and_apply(|audit| {
        let id = ordered(audit)[1];
        audit.review::<Text>(
            id,
            Review::Redact {
                policy_id: POLICY_ID,
                action: mask(),
            },
        );
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
        let id = ordered(audit)[0];
        audit.review::<Text>(
            id,
            Review::Redact {
                policy_id: POLICY_ID,
                action: mask(),
            },
        );
        audit.review::<Text>(
            id,
            Review::Suppress {
                reason: Some("actually fine".into()),
                actor: None,
            },
        );
        assert!(audit.is_suppressed::<Text>(id));
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
        let ids = ordered(audit);
        audit.review::<Text>(
            ids[0],
            Review::Suppress {
                reason: Some("false positive".into()),
                actor: None,
            },
        );
        audit.review::<Text>(
            ids[1],
            Review::Redact {
                policy_id: POLICY_ID,
                action: mask(),
            },
        );
        audit
            .report
            .include::<Text>(manual_entity(span_of(b"SECRET-9")));
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
    let engine = Engine::new(ProviderConfig::default().build());
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
    let posted_back = round_trip(&engine, &json);

    for entity in posted_back.report.entities::<Text>().expect("a text body") {
        let selection = entity
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
            entity.audit.verify().is_ok(),
            "recording a pick keeps the hash chain intact",
        );
    }
}

#[tokio::test]
async fn a_suppression_supersedes_the_pick_before_it() {
    // The pick is recorded at analyze, before any reviewer has seen
    // it, so a later suppression cannot un-record it. What matters is
    // that the suppression is the *newer* event and that no further
    // pick lands after it: the trail reads "we would have erased
    // this, then a human said leave it", which is the history, and
    // the entity is skipped.
    let engine = Engine::new(ProviderConfig::default().build());
    let policy = policy();
    let mut audit = engine
        .analyze(
            doc(),
            std::slice::from_ref(&policy),
            &AnalyzerParams::default(),
        )
        .await
        .expect("analyze");

    let target = ordered(&audit)[0];
    assert!(
        entity(&audit, target).audit.selection().is_some(),
        "precondition: analyze recorded a pick",
    );

    audit.review::<Text>(
        target,
        Review::Suppress {
            reason: Some("false positive".into()),
            actor: None,
        },
    );
    engine
        .anonymize(
            doc(),
            std::slice::from_ref(&policy),
            &mut audit,
            &RequestContext::new(),
        )
        .await
        .expect("anonymize");

    let record = entity(&audit, target);
    assert!(
        record.is_suppressed(),
        "the suppression is what holds after apply",
    );

    // No Redaction event: nothing ran on it, despite the earlier pick.
    assert!(
        record.audit.redaction().is_none(),
        "a suppressed entity is never redacted, whatever its pick said",
    );

    // And the suppression is the last word on the trail.
    let last_decision = record
        .audit
        .events()
        .iter()
        .rev()
        .find_map(|e| match &e.kind {
            AuditKind::Manual(m) => Some(format!("manual:{:?}", m.intent)),
            AuditKind::Selection(_) => Some("selection".to_owned()),
            _ => None,
        });
    assert_eq!(
        last_decision.as_deref(),
        Some("manual:Suppress"),
        "no pick is recorded after the suppression",
    );
}

#[tokio::test]
async fn a_reviewer_can_take_a_suppression_back() {
    // Suppress, apply, then change your mind. The first apply stamps
    // a Manual event on the trail, and `is_suppressed` reads the
    // trail — so without reconciling the state, the entity stays
    // skipped and the redaction silently never happens.
    let engine = Engine::new(ProviderConfig::default().build());
    let policy = policy();
    let mut audit = engine
        .analyze(
            doc(),
            std::slice::from_ref(&policy),
            &AnalyzerParams::default(),
        )
        .await
        .expect("analyze");

    let target = ordered(&audit)[0];
    audit.review::<Text>(
        target,
        Review::Suppress {
            reason: Some("false positive".into()),
            actor: None,
        },
    );
    let first = engine
        .anonymize(
            doc(),
            std::slice::from_ref(&policy),
            &mut audit,
            &RequestContext::new(),
        )
        .await
        .expect("anonymize");
    assert!(
        String::from_utf8_lossy(&first.bytes).contains("alice@example.com"),
        "the suppression holds on the first pass",
    );

    // Round-trip, as a host would, then reverse the decision.
    let json = serde_json::to_string(&audit).expect("serializes");
    let mut posted_back = round_trip(&engine, &json);
    assert!(
        posted_back.is_suppressed::<Text>(target),
        "the applied suppression survives the round-trip",
    );
    posted_back.review::<Text>(
        target,
        Review::Redact {
            policy_id: POLICY_ID,
            action: TextRedaction::Erase,
        },
    );
    assert!(
        !posted_back.is_suppressed::<Text>(target),
        "a pending redact supersedes the recorded suppression",
    );

    let second = engine
        .anonymize(
            doc(),
            std::slice::from_ref(&policy),
            &mut posted_back,
            &RequestContext::new(),
        )
        .await
        .expect("anonymize");
    let out = String::from_utf8_lossy(&second.bytes);
    assert!(
        !out.contains("alice@example.com"),
        "reversing a suppression must actually redact: {out}",
    );

    // The reversal is auditable: both halves of the change of mind
    // stay on the trail rather than the suppression being erased.
    let reviewed = entity(&posted_back, target);
    let manual_intents: Vec<_> = reviewed
        .audit
        .events()
        .iter()
        .filter_map(|e| match &e.kind {
            AuditKind::Manual(m) => Some(m.intent),
            _ => None,
        })
        .collect();
    assert_eq!(
        manual_intents.len(),
        2,
        "the suppression and its reversal are both recorded: {manual_intents:?}",
    );
    assert!(
        reviewed.audit.verify().is_ok(),
        "the hash chain survives the reversal",
    );
}

#[tokio::test]
async fn re_applying_an_audit_does_not_stack_manual_events() {
    // `apply_suppressions` runs on every anonymize, so a host that
    // re-applies the same audit must not grow the trail each time.
    let engine = Engine::new(ProviderConfig::default().build());
    let policy = policy();
    let mut audit = engine
        .analyze(
            doc(),
            std::slice::from_ref(&policy),
            &AnalyzerParams::default(),
        )
        .await
        .expect("analyze");

    let target = ordered(&audit)[0];
    audit.review::<Text>(
        target,
        Review::Suppress {
            reason: None,
            actor: None,
        },
    );
    for _ in 0..3 {
        engine
            .anonymize(
                doc(),
                std::slice::from_ref(&policy),
                &mut audit,
                &RequestContext::new(),
            )
            .await
            .expect("anonymize");
    }

    let manual_count = entity(&audit, target)
        .audit
        .events()
        .iter()
        .filter(|e| matches!(e.kind, AuditKind::Manual(_)))
        .count();
    assert_eq!(
        manual_count, 1,
        "three applies of one suppression leave one Manual event",
    );
}
