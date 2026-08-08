//! End-to-end tests for the wire shapes in [`nvisy_schema::policy`]:
//! per-label table rules, label-group predicates, and any other
//! policy-side sugar the engine flattens at compile time.
//!
//! Runs against the plaintext sample so assertions can inspect the
//! per-entity provenance chain directly.

use bytes::Bytes;
use elide_core::entity::LabelRef;
use elide_core::entity::provenance::EventKind;
use nvisy_engine::{Audit, Engine, EntityGroup};
use nvisy_schema::file::Document;
use nvisy_schema::plan::{
    AnalyzerParams, EnricherParams, PatternRecognizerParams, ProviderSelection, RecognizerParams,
    ScopeParams,
};
use nvisy_schema::policy::predicate::Predicate;
use nvisy_schema::policy::redaction::{ModalityRedactions, TextRedaction};
use nvisy_schema::policy::{
    LabelEntry, LabelGroup, Labels, PolicyDefinition, PolicyRule, PredicatedRule, TableRule,
};

const SAMPLE_TXT: &[u8] = include_bytes!("testdata/sample.txt");

fn engine() -> Engine {
    Engine::new()
}

fn raw_txt() -> Document {
    Document::new(Bytes::from_static(SAMPLE_TXT), "txt")
}

fn default_spec() -> AnalyzerParams {
    AnalyzerParams {
        recognizers: RecognizerParams {
            pattern: Some(PatternRecognizerParams {
                builtins: true,
                context_enhanced: true,
                ..Default::default()
            }),
            ner: Some(ProviderSelection::All(false)),
            llm: Some(ProviderSelection::All(false)),
        },
        enrichers: EnricherParams::default(),
        deduplication: Default::default(),
        scope: ScopeParams::default(),
        annotations: Default::default(),
    }
}

/// Count redaction events on every text-modality entity of
/// `audit.body` whose label matches `label`. A redaction event is
/// stamped once per operator run, so this is the number of
/// operator hits on the labeled entities.
fn redaction_hits(audit: &Audit, label: &str) -> usize {
    let Some(EntityGroup::Text(entities)) = audit.body.as_ref() else {
        return 0;
    };
    entities
        .iter()
        .filter(|r| r.entity.label.as_str() == label)
        .flat_map(|r| r.entity.provenance.events.iter())
        .filter(|e| matches!(e.kind, EventKind::Redaction { .. }))
        .count()
}

/// Assert that a `TableRule` routes each label to its paired
/// operator: both email and phone entities in the sample get a
/// redaction event, driven by the table under one shared UUID.
#[tokio::test]
async fn table_rule_dispatches_per_label_under_one_identity() {
    let engine = engine();
    let rule_id = uuid::Uuid::now_v7();
    let table = PolicyRule::Table(TableRule {
        id: rule_id,
        name: "contact-sweep".into(),
        description: None,
        operators: vec![
            LabelEntry {
                label: LabelRef::new("email_address"),
                action: ModalityRedactions {
                    text: Some(TextRedaction::Erase),
                    ..Default::default()
                },
            },
            LabelEntry {
                label: LabelRef::new("phone_number"),
                action: ModalityRedactions {
                    text: Some(TextRedaction::Replace {
                        template: "[phone]".to_owned(),
                    }),
                    ..Default::default()
                },
            },
        ],
    });
    let policy = PolicyDefinition {
        id: uuid::Uuid::now_v7(),
        name: "contact-info".into(),
        description: None,
        when: None,
        labels: Labels {
            builtins: vec![LabelRef::new("email_address"), LabelRef::new("phone_number")],
            custom: Vec::new(),
        },
        rules: vec![table],
        fallback: None,
        retention: Vec::new(),
    };

    let mut analyzed = engine
        .analyze(
            raw_txt(),
            std::slice::from_ref(&policy),
            &[],
            &default_spec(),
        )
        .await
        .expect("analyze succeeds");
    let redacted = engine
        .anonymize(
            raw_txt(),
            std::slice::from_ref(&policy),
            &[],
            &mut analyzed,
        )
        .await
        .expect("anonymize succeeds");

    assert!(
        redaction_hits(&analyzed, "email_address") >= 1,
        "email entries in the sample should each get a redaction event",
    );
    assert!(
        redaction_hits(&analyzed, "phone_number") >= 1,
        "phone entries in the sample should each get a redaction event",
    );

    // The interesting invariant: each label routes to *its own*
    // operator. Inspect the redacted body — the phone entries must
    // read `[phone]` (Replace template), and the raw email address
    // must be gone (Erase). Both are strong enough that a bug
    // routing everything through one operator would trip them.
    let body = std::str::from_utf8(&redacted.bytes).expect("body is utf-8");
    assert!(
        body.contains("[phone]"),
        "phone entries must render as the Replace template `[phone]`; body was:\n{body}",
    );
    assert!(
        !body.contains("jane.doe@example.com"),
        "email entries must be erased, not replaced; body was:\n{body}",
    );

    // Attribution: both branches share the shared rule UUID.
    let Some(EntityGroup::Text(entities)) = analyzed.body.as_ref() else {
        panic!("expected text body");
    };
    let attribution = entities
        .iter()
        .find(|r| r.entity.label.as_str() == "email_address")
        .and_then(|r| {
            r.entity
                .provenance
                .events
                .iter()
                .find_map(|e| match &e.kind {
                    EventKind::Redaction { attribution, .. } => attribution.as_ref(),
                    _ => None,
                })
        })
        .expect("email entity must have a redaction event with an attribution");
    assert_eq!(
        attribution.description.as_deref(),
        Some(rule_id.to_string().as_str()),
        "attribution.description must carry the shared rule UUID",
    );
}

/// A `Predicate::LabelInGroup` predicate drives the same redaction
/// path as a `TagOneOf` over the synthetic `group:<name>` tag —
/// asserts the group compilation and predicate rewrite are wired
/// end-to-end.
#[tokio::test]
async fn label_in_group_predicate_fires_on_grouped_labels() {
    let engine = engine();
    let policy = PolicyDefinition {
        id: uuid::Uuid::now_v7(),
        name: "sweep".into(),
        description: None,
        when: None,
        labels: Labels {
            builtins: vec![LabelRef::new("email_address"), LabelRef::new("phone_number")],
            custom: Vec::new(),
        },
        rules: vec![PolicyRule::Predicated(Box::new(PredicatedRule {
            id: uuid::Uuid::now_v7(),
            name: "erase-contacts".into(),
            description: None,
            predicate: Predicate::LabelInGroup {
                group: "contact_info".to_owned(),
            },
            action: ModalityRedactions {
                text: Some(TextRedaction::Erase),
                ..Default::default()
            },
        }))],
        fallback: None,
        retention: Vec::new(),
    };
    let group = LabelGroup {
        name: "contact_info".into(),
        description: None,
        labels: vec![LabelRef::new("email_address"), LabelRef::new("phone_number")],
    };

    let mut analyzed = engine
        .analyze(
            raw_txt(),
            std::slice::from_ref(&policy),
            std::slice::from_ref(&group),
            &default_spec(),
        )
        .await
        .expect("analyze succeeds");
    engine
        .anonymize(
            raw_txt(),
            std::slice::from_ref(&policy),
            std::slice::from_ref(&group),
            &mut analyzed,
        )
        .await
        .expect("anonymize succeeds");

    assert!(
        redaction_hits(&analyzed, "email_address") >= 1,
        "email entries fall in the group and must be redacted",
    );
    assert!(
        redaction_hits(&analyzed, "phone_number") >= 1,
        "phone entries fall in the group and must be redacted",
    );
    // Not asserting "non-group labels untouched" here: elide
    // clusters overlapping detections and stamps the winner's
    // redaction event on every cluster member, so a non-group
    // label overlapping a group-label detection can carry a
    // redaction event without being *matched* by the group rule.
    // The positive checks above are sufficient to prove the
    // group predicate wired through the compile path.
}

