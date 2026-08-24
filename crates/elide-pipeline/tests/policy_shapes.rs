//! End-to-end tests for the wire shapes in [`elide_governance`]:
//! per-label table rules, label-group predicates, and any other
//! policy-side sugar the engine flattens at compile time.
//!
//! Runs against the plaintext sample so assertions can inspect the
//! per-entity provenance chain directly.

use bytes::Bytes;
use elide::entity::LabelRef;
use elide::entity::audit::AuditKind;
use elide::modality::text::Text;
use elide_governance::redaction::{ModalityRedactions, TextRedaction};
use elide_governance::{
    LabelEntry, LabelScope, PolicyDefinition, PolicyRule, Predicate, RuleDispatch,
};
use elide_pipeline::entity::Review;
use elide_pipeline::file::Document;
use elide_pipeline::{Audit, Engine, ProviderConfig, RequestContext};

const SAMPLE_TXT: &[u8] = include_bytes!("testdata/sample.txt");

fn engine() -> Engine {
    Engine::new(ProviderConfig::default().build())
}

fn raw_txt() -> Document {
    Document::new(Bytes::from_static(SAMPLE_TXT), "txt")
}

fn default_spec() -> RequestContext {
    RequestContext::new()
}

/// Count redaction events on every text-modality entity of
/// the audit's report whose label matches `label`. A redaction event is
/// stamped once per operator run, so this is the number of
/// operator hits on the labeled entities.
fn redaction_hits(audit: &Audit, label: &str) -> usize {
    let Some(entities) = audit.report.entities::<Text>() else {
        return 0;
    };
    entities
        .iter()
        .filter(|e| e.label.as_str() == label)
        .flat_map(|e| e.audit.events().iter())
        .filter(|e| matches!(e.kind, AuditKind::Redaction(_)))
        .count()
}

/// Assert that a `RuleDispatch` routes each label to its paired
/// operator: both email and phone entities in the sample get a
/// redaction event, driven by the table under one shared UUID.
#[tokio::test]
async fn table_rule_dispatches_per_label_under_one_identity() {
    let engine = engine();
    let rule_id = uuid::Uuid::now_v7();
    let table = PolicyRule {
        id: rule_id,
        name: "contact-sweep".into(),
        description: None,
        attribution: None,
        dispatch: RuleDispatch::Table {
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
        },
    };
    let policy = PolicyDefinition {
        id: uuid::Uuid::now_v7(),
        name: "contact-info".into(),
        description: None,
        template: None,
        // The table targets both; the scope must detect both.
        scopes: vec![LabelScope::new(
            "scope",
            vec![
                LabelRef::new("email_address"),
                LabelRef::new("phone_number"),
            ],
        )],
        custom: Vec::new(),
        rules: vec![table],
        fallback: None,
    };

    let mut analyzed = engine
        .analyze(raw_txt(), std::slice::from_ref(&policy), &default_spec())
        .await
        .expect("analyze succeeds");
    let redacted = engine
        .anonymize(
            raw_txt(),
            std::slice::from_ref(&policy),
            &mut analyzed,
            None,
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
    // operator. Inspect the redacted body: the phone entries must
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
    let Some(entities) = analyzed.report.entities::<Text>() else {
        panic!("expected text body");
    };
    let attribution = entities
        .iter()
        .find(|e| e.label.as_str() == "email_address")
        .and_then(|entity| {
            entity
                .audit
                .events()
                .iter()
                .find_map(|event| match &event.kind {
                    AuditKind::Redaction(r) => r.attribution.as_ref(),
                    _ => None,
                })
        })
        .expect("email entity must have a redaction event with an attribution");
    assert_eq!(
        attribution.source_id,
        Some(rule_id),
        "attribution.source_id must carry the shared rule UUID",
    );
}

/// A `Predicate::LabelInScope` predicate drives the same redaction
/// path as a `TagOneOf` over the synthetic `group:<name>` tag -
/// asserts the group compilation and predicate rewrite are wired
/// end-to-end.
#[tokio::test]
async fn label_in_group_predicate_fires_on_grouped_labels() {
    let engine = engine();
    let group = LabelScope {
        name: "contact_info".into(),
        description: None,
        attribution: None,
        labels: vec![
            LabelRef::new("email_address"),
            LabelRef::new("phone_number"),
        ],
    };
    let policy = PolicyDefinition {
        id: uuid::Uuid::now_v7(),
        name: "sweep".into(),
        description: None,
        template: None,
        scopes: vec![group],
        custom: Vec::new(),
        rules: vec![PolicyRule {
            id: uuid::Uuid::now_v7(),
            name: "erase-contacts".into(),
            description: None,
            attribution: None,
            dispatch: RuleDispatch::Predicated {
                predicate: Predicate::LabelInScope {
                    scope: "contact_info".to_owned(),
                },
                action: Box::new(ModalityRedactions {
                    text: Some(TextRedaction::Erase),
                    ..Default::default()
                }),
            },
        }],
        fallback: None,
    };

    let mut analyzed = engine
        .analyze(raw_txt(), std::slice::from_ref(&policy), &default_spec())
        .await
        .expect("analyze succeeds");
    engine
        .anonymize(
            raw_txt(),
            std::slice::from_ref(&policy),
            &mut analyzed,
            None,
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

/// Per-policy label scoping: a rule inside policy A cannot fire
/// on an entity whose label another policy contributed to the
/// request-wide recognition pass. Policy A lists only `email_address`;
/// its `TagOneOf { tags: ["pii"] }` rule must NOT match `phone_number`
/// entities that policy B enabled.
#[tokio::test]
async fn per_policy_label_scoping_blocks_cross_policy_tag_bleed() {
    let engine = engine();
    let policy_a = PolicyDefinition {
        id: uuid::Uuid::now_v7(),
        name: "email-only".into(),
        description: None,
        template: None,
        // A TagOneOf rule names no label, so the scope is what
        // bounds which entities it can reach.
        scopes: vec![LabelScope::new(
            "scope",
            vec![LabelRef::new("email_address")],
        )],
        custom: Vec::new(),
        rules: vec![PolicyRule {
            id: uuid::Uuid::now_v7(),
            name: "erase-pii".into(),
            description: None,
            attribution: None,
            dispatch: RuleDispatch::Predicated {
                predicate: Predicate::TagOneOf {
                    tags: vec!["pii".to_owned()],
                },
                action: Box::new(ModalityRedactions {
                    text: Some(TextRedaction::Erase),
                    ..Default::default()
                }),
            },
        }],
        fallback: None,
    };
    let policy_b = PolicyDefinition {
        id: uuid::Uuid::now_v7(),
        name: "phone-only-no-rules".into(),
        description: None,
        template: None,
        // Policy B contributes `phone_number` to the request's
        // recognition vocabulary. Without it the entity is never
        // detected and the zero-hit assertion below would pass for
        // the wrong reason.
        scopes: vec![LabelScope::new(
            "scope",
            vec![LabelRef::new("phone_number")],
        )],
        custom: Vec::new(),
        // No rules: policy B only contributes vocabulary.
        rules: Vec::new(),
        fallback: None,
    };

    let policies = vec![policy_a, policy_b];
    let mut analyzed = engine
        .analyze(raw_txt(), &policies, &default_spec())
        .await
        .expect("analyze succeeds");
    engine
        .anonymize(raw_txt(), &policies, &mut analyzed, None)
        .await
        .expect("anonymize succeeds");

    assert!(
        redaction_hits(&analyzed, "email_address") >= 1,
        "policy A declares email_address and matches its own pii \
         tag: email must be redacted",
    );
    assert_eq!(
        redaction_hits(&analyzed, "phone_number"),
        0,
        "policy A does NOT declare phone_number; its tag-based rule \
         must not fire on B's vocabulary. Per-policy scoping is the \
         invariant.",
    );
}

/// Fallbacks compose across policies via a two-pass attach:
/// every policy's rules attach first, then every policy's
/// fallback. A coarse baseline policy's fallback must NOT
/// shadow a subsequent specific policy's rule on the same
/// label; the specific rule wins because it was attached
/// before the fallback in the elide rule chain.
#[tokio::test]
async fn coarse_fallback_does_not_shadow_specific_later_rule() {
    let engine = engine();
    let coarse = PolicyDefinition {
        id: uuid::Uuid::now_v7(),
        name: "coarse-baseline".into(),
        description: None,
        template: None,
        // The fallback sweeps whatever this scope detects.
        scopes: vec![LabelScope::new(
            "scope",
            vec![
                LabelRef::new("email_address"),
                LabelRef::new("phone_number"),
            ],
        )],
        custom: Vec::new(),
        rules: Vec::new(),
        fallback: Some(ModalityRedactions {
            // Coarse baseline says "erase" as a catch-all.
            text: Some(TextRedaction::Erase),
            ..Default::default()
        }),
    };
    let specific = PolicyDefinition {
        id: uuid::Uuid::now_v7(),
        name: "specific-refinement".into(),
        description: None,
        template: None,
        // Refines just email; the coarse policy keeps the rest.
        scopes: vec![LabelScope::new(
            "scope",
            vec![LabelRef::new("email_address")],
        )],
        custom: Vec::new(),
        rules: vec![PolicyRule {
            id: uuid::Uuid::now_v7(),
            name: "replace-email".into(),
            description: None,
            attribution: None,
            dispatch: RuleDispatch::Predicated {
                predicate: Predicate::LabelOneOf {
                    labels: vec![LabelRef::new("email_address")],
                },
                action: Box::new(ModalityRedactions {
                    text: Some(TextRedaction::Replace {
                        template: "[replaced-email]".to_owned(),
                    }),
                    ..Default::default()
                }),
            },
        }],
        fallback: None,
    };

    let policies = vec![coarse, specific];
    let mut analyzed = engine
        .analyze(raw_txt(), &policies, &default_spec())
        .await
        .expect("analyze succeeds");
    let redacted = engine
        .anonymize(raw_txt(), &policies, &mut analyzed, None)
        .await
        .expect("anonymize succeeds");

    let body = std::str::from_utf8(&redacted.bytes).expect("utf-8");
    assert!(
        body.contains("[replaced-email]"),
        "specific policy's Replace rule must win over the coarse policy's \
         Erase fallback because fallbacks attach after every policy's rules \
         (two-pass). Body:\n{body}",
    );
    assert!(
        !body.contains("jane.doe@example.com"),
        "the original email must be replaced (not left intact). Body:\n{body}",
    );
}

/// The engine rejects a rule inside policy A that references a
/// group name only policy B declares: groups are per-policy
/// namespaces (strict scoping), enforced at request-compile
/// before any redaction runs.
#[tokio::test]
async fn cross_policy_group_reference_fails_the_request() {
    let engine = engine();
    let policy_a = PolicyDefinition {
        id: uuid::Uuid::now_v7(),
        name: "borrower".into(),
        description: None,
        template: None,
        // Scope carries what this policy detects.
        scopes: vec![LabelScope::new(
            "scope",
            vec![LabelRef::new("email_address")],
        )],
        // No groups declared here.
        custom: Vec::new(),
        rules: vec![PolicyRule {
            id: uuid::Uuid::now_v7(),
            name: "borrow-from-b".into(),
            description: None,
            attribution: None,
            dispatch: RuleDispatch::Predicated {
                predicate: Predicate::LabelInScope {
                    scope: "contact_info".to_owned(),
                },
                action: Box::new(ModalityRedactions {
                    text: Some(TextRedaction::Erase),
                    ..Default::default()
                }),
            },
        }],
        fallback: None,
    };
    let policy_b = PolicyDefinition {
        id: uuid::Uuid::now_v7(),
        name: "declares-group".into(),
        description: None,
        template: None,
        scopes: vec![LabelScope::new(
            "contact_info",
            vec![LabelRef::new("email_address")],
        )],
        custom: Vec::new(),
        rules: Vec::new(),
        fallback: None,
    };

    // `Audit` is not `Debug` (it holds an elide `Report`), so the
    // error is matched out rather than `expect_err`ed.
    let Err(err) = engine
        .analyze(raw_txt(), &[policy_a, policy_b], &default_spec())
        .await
    else {
        panic!("policy A must not reach into policy B's groups");
    };
    let msg = err.to_string();
    assert!(
        msg.contains("contact_info"),
        "error must name the unresolved group; got: {msg}",
    );
}

/// Reviewer overrides carry a `policy_id` naming the policy
/// whose authority the override exercises. Anonymize must reject
/// a request whose override names a policy no submitted policy
/// carries: that authority doesn't exist in the request, and
/// silently attributing to nothing (or falling back to engine
/// defaults) would misroute per-policy operator infrastructure.
#[tokio::test]
async fn override_naming_unknown_policy_fails_the_request() {
    let engine = engine();
    let policy = PolicyDefinition {
        id: uuid::Uuid::now_v7(),
        name: "authorising".into(),
        description: None,
        template: None,
        scopes: Vec::new(),
        custom: Vec::new(),
        rules: Vec::new(),
        fallback: None,
    };
    let mut analyzed = engine
        .analyze(raw_txt(), std::slice::from_ref(&policy), &default_spec())
        .await
        .expect("analyze succeeds");
    let target = analyzed
        .report
        .entities::<Text>()
        .expect("expected text body")[0]
        .id;
    // Ship an override that names a policy id no one submitted.
    analyzed.review::<Text>(
        target,
        Review::Redact {
            policy_id: uuid::Uuid::now_v7(),
            action: TextRedaction::Erase,
        },
    );

    let err = engine
        .anonymize(
            raw_txt(),
            std::slice::from_ref(&policy),
            &mut analyzed,
            None,
        )
        .await
        .expect_err("override with unknown policy authority must be rejected");
    let msg = err.to_string();
    assert!(
        msg.contains("no submitted policy") || msg.contains("policy"),
        "error must explain the missing authority; got: {msg}",
    );
}

/// Two scopes sharing a name make `LabelInScope` resolve one
/// labelset while `label_scope()` unions both, so recognition and
/// redaction would disagree about what the name means. Reject the
/// request instead.
#[tokio::test]
async fn duplicate_scope_names_fail_the_request() {
    let engine = engine();
    let policy = PolicyDefinition {
        id: uuid::Uuid::now_v7(),
        name: "ambiguous".into(),
        description: None,
        template: None,
        scopes: vec![
            LabelScope::new("contact", vec![LabelRef::new("email_address")]),
            LabelScope::new("contact", vec![LabelRef::new("phone_number")]),
        ],
        custom: Vec::new(),
        rules: vec![PolicyRule {
            id: uuid::Uuid::now_v7(),
            name: "erase-contact".into(),
            description: None,
            attribution: None,
            dispatch: RuleDispatch::Predicated {
                predicate: Predicate::LabelInScope {
                    scope: "contact".to_owned(),
                },
                action: Box::new(ModalityRedactions {
                    text: Some(TextRedaction::Erase),
                    ..Default::default()
                }),
            },
        }],
        fallback: None,
    };

    let Err(err) = engine
        .analyze(raw_txt(), std::slice::from_ref(&policy), &default_spec())
        .await
    else {
        panic!("a duplicate scope name must reject the request");
    };
    let msg = err.to_string();
    assert!(msg.contains("contact"), "error must name the scope: {msg}");
    assert!(
        msg.contains("more than once"),
        "error must say why it was rejected: {msg}",
    );
}

/// A policy that redacts entirely through its fallback still cites
/// the authority its scope carries. Without this, CCPA and GDPR
/// would lose their regulatory attribution: every one of their
/// redactions runs through the fallback.
#[tokio::test]
async fn fallback_carries_the_scope_attribution() {
    use elide::entity::audit::AttributionKind;

    let engine = engine();
    let cited = AttributionKind::Cited {
        authority: "CCPA".into(),
        citation: "Cal. Civ. Code §1798.140(v)(1)".into(),
        rationale: "personal information".into(),
    };
    let policy = PolicyDefinition {
        id: uuid::Uuid::now_v7(),
        name: "sweep-everything".into(),
        description: None,
        template: None,
        scopes: vec![
            LabelScope::new("pi", vec![LabelRef::new("email_address")])
                .with_attribution(cited.clone()),
        ],
        custom: Vec::new(),
        rules: Vec::new(),
        fallback: Some(ModalityRedactions {
            text: Some(TextRedaction::Erase),
            ..Default::default()
        }),
    };

    let mut analyzed = engine
        .analyze(raw_txt(), std::slice::from_ref(&policy), &default_spec())
        .await
        .expect("analyze succeeds");
    engine
        .anonymize(
            raw_txt(),
            std::slice::from_ref(&policy),
            &mut analyzed,
            None,
        )
        .await
        .expect("anonymize succeeds");

    let Some(entities) = analyzed.report.entities::<Text>() else {
        panic!("expected text body");
    };
    let attribution = entities
        .iter()
        .find(|e| e.label.as_str() == "email_address")
        .and_then(|entity| {
            entity
                .audit
                .events()
                .iter()
                .find_map(|event| match &event.kind {
                    AuditKind::Redaction(r) => r.attribution.as_ref(),
                    _ => None,
                })
        })
        .expect("the fallback must stamp an attribution");
    assert_eq!(
        attribution.kind, cited,
        "fallback must cite the scope's authority, not a generic label",
    );
}

/// A cited scope beside an uncited one is ambiguous: stamping the
/// citation on entities from the uncited scope would attribute a
/// redaction to an authority that does not cover it. The fallback
/// falls back to the policy's own name instead.
#[tokio::test]
async fn mixed_scope_attribution_does_not_borrow_a_citation() {
    use elide::entity::audit::AttributionKind;

    let engine = engine();
    let policy = PolicyDefinition {
        id: uuid::Uuid::now_v7(),
        name: "half-cited".into(),
        description: None,
        template: None,
        scopes: vec![
            LabelScope::new("cited", vec![LabelRef::new("email_address")]).with_attribution(
                AttributionKind::Cited {
                    authority: "CCPA".into(),
                    citation: "Cal. Civ. Code §1798.140(v)(1)".into(),
                    rationale: "personal information".into(),
                },
            ),
            // No attribution: `phone_number` answers to nothing the
            // policy declared.
            LabelScope::new("uncited", vec![LabelRef::new("phone_number")]),
        ],
        custom: Vec::new(),
        rules: Vec::new(),
        fallback: Some(ModalityRedactions {
            text: Some(TextRedaction::Erase),
            ..Default::default()
        }),
    };

    let mut analyzed = engine
        .analyze(raw_txt(), std::slice::from_ref(&policy), &default_spec())
        .await
        .expect("analyze succeeds");
    engine
        .anonymize(
            raw_txt(),
            std::slice::from_ref(&policy),
            &mut analyzed,
            None,
        )
        .await
        .expect("anonymize succeeds");

    let Some(entities) = analyzed.report.entities::<Text>() else {
        panic!("expected text body");
    };
    let attribution = entities
        .iter()
        .find(|e| e.label.as_str() == "phone_number")
        .and_then(|entity| {
            entity
                .audit
                .events()
                .iter()
                .find_map(|event| match &event.kind {
                    AuditKind::Redaction(r) => r.attribution.as_ref(),
                    _ => None,
                })
        })
        .expect("the fallback must stamp an attribution");
    assert!(
        matches!(&attribution.kind, AttributionKind::Freeform { name, .. } if name == "half-cited"),
        "a mixed-attribution policy must not borrow one scope's citation, got: {:?}",
        attribution.kind,
    );
}
