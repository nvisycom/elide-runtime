//! End-to-end at the engine layer over a DOCX (text body +
//! embedded PNG): analyze, override one entity, anonymize,
//! assert the body changed and the image part round-tripped
//! unchanged.

mod fixtures;

use std::io::{Cursor, Read};

use bytes::Bytes;
use elide_core::entity::LabelRef;
use elide_governance::redaction::{ModalityRedactions, TextRedaction};
use elide_governance::{Labels, PolicyDefinition};
use elide_pipeline::entity::Review;
use elide_pipeline::provider::{OcrBackend, OcrConfig, OcrEnricherConfig};
use elide_pipeline::{Audit, AuditContext, Engine, EntityGroup};
use elide_wire::file::Document;
use elide_wire::plan::AnalyzerParams;

use self::fixtures::write_artefact;

const SAMPLE_DOCX: &[u8] = include_bytes!("testdata/sample.docx");
const IMAGE_PART_ID: &str = "word/media/image1.png";

fn raw_docx() -> Document {
    Document::new(Bytes::from_static(SAMPLE_DOCX), "docx")
}

fn engine() -> Engine {
    Engine::new().with_ocr(OcrConfig {
        enrichers: vec![OcrEnricherConfig {
            name: "mock".into(),
            description: None,
            backend: OcrBackend::Mock,
        }],
    })
}

fn default_spec() -> AnalyzerParams {
    AnalyzerParams::default()
}

fn read_zip_entry(buf: &[u8], name: &str) -> Option<Vec<u8>> {
    let mut zip = zip::ZipArchive::new(Cursor::new(buf.to_vec())).ok()?;
    let mut entry = zip.by_name(name).ok()?;
    let mut out = Vec::new();
    entry.read_to_end(&mut out).ok()?;
    Some(out)
}

#[tokio::test]
async fn analyze_captures_text_body_and_image_part() {
    let engine = engine();
    let analyzed = engine
        .analyze(raw_docx(), &[], &default_spec())
        .await
        .expect("analyze succeeds");

    let body_group = analyzed.body.as_ref().expect("body group present");
    let text_entities = match body_group {
        EntityGroup::Text(entities) => entities,
        other => panic!("expected Text body, got {other:?}"),
    };
    assert!(
        !text_entities.is_empty(),
        "fixture should carry at least one body entity",
    );

    let image_group = analyzed.parts.get(IMAGE_PART_ID).unwrap_or_else(|| {
        panic!(
            "expected part `{IMAGE_PART_ID}` in analyzed.parts; got keys: {:?}",
            analyzed.parts.keys().collect::<Vec<_>>(),
        )
    });
    assert!(
        matches!(image_group, EntityGroup::Image(_)),
        "expected Image variant for image part, got {image_group:?}",
    );
}

#[tokio::test]
async fn anonymize_redacts_targeted_entity_and_preserves_other_parts() {
    let engine = engine();
    let mut analyzed = engine
        .analyze(raw_docx(), &[], &default_spec())
        .await
        .expect("analyze succeeds");
    let body_group = analyzed.body.as_ref().expect("body group present");
    let EntityGroup::Text(entities) = body_group else {
        panic!("expected Text body");
    };
    assert!(
        !entities.is_empty(),
        "fixture should carry at least one entity"
    );

    // Reviewer overrides carry a policy authority. Ship a
    // minimal policy the override can attribute to (no rules,
    // no fallback — it exists only so the override validator
    // accepts the request).
    let review_policy = PolicyDefinition {
        id: uuid::Uuid::now_v7(),
        name: "review-authority".into(),
        description: None,
        labels: Labels::default(),
        groups: Vec::new(),
        rules: Vec::new(),
        fallback: None,
    };
    let target_id = entities[0].entity.id;
    let EntityGroup::Text(muts) = analyzed.body.as_mut().unwrap() else {
        unreachable!()
    };
    muts.iter_mut()
        .find(|r| r.entity.id == target_id)
        .expect("entity present")
        .review = Some(Review {
        policy_id: review_policy.id,
        action: ModalityRedactions {
            text: Some(TextRedaction::Erase),
            tabular: None,
            image: None,
            audio: None,
        },
    });

    let outcome = engine
        .anonymize(
            raw_docx(),
            std::slice::from_ref(&review_policy),
            &mut analyzed,
        )
        .await
        .expect("anonymize succeeds");
    write_artefact("sample", "out.docx", &outcome.bytes);

    let original_body =
        read_zip_entry(SAMPLE_DOCX, "word/document.xml").expect("fixture has word/document.xml");
    let redacted_body = read_zip_entry(&outcome.bytes, "word/document.xml")
        .expect("redacted docx still has word/document.xml");
    assert_ne!(
        redacted_body, original_body,
        "Erase override must change the body XML",
    );

    let image_bytes =
        read_zip_entry(&outcome.bytes, IMAGE_PART_ID).expect("image part survives apply");
    let original_image =
        read_zip_entry(SAMPLE_DOCX, IMAGE_PART_ID).expect("fixture has the image part");
    assert_eq!(
        image_bytes, original_image,
        "image part must round-trip unchanged when no override targets it",
    );
}

#[tokio::test]
async fn audit_context_mirrors_spec_scope_and_carries_correlation_id() {
    let engine = engine();

    let mut spec = default_spec();
    spec.scope.metadata.tags = vec!["gdpr-request".into()];
    spec.scope.metadata.purpose = Some("dsar-response".into());
    spec.scope.metadata.audience = vec!["data-subject".into(), "compliance-review".into()];
    let doc = raw_docx();
    let correlation_id = doc.correlation_id;

    let audit = engine
        .analyze(doc, &[], &spec)
        .await
        .expect("analyze succeeds");

    assert_eq!(
        audit.context.metadata.tags, spec.scope.metadata.tags,
        "audit context must mirror the caller-asserted scope tags",
    );
    assert_eq!(
        audit.context.metadata.purpose, spec.scope.metadata.purpose,
        "audit context must mirror the caller-asserted scope purpose",
    );
    assert_eq!(
        audit.context.metadata.audience, spec.scope.metadata.audience,
        "audit context must mirror the caller-asserted scope audience",
    );
    assert_eq!(
        audit.context.correlation_id, correlation_id,
        "analyze-time correlation id must be recorded on the audit context",
    );
}

#[tokio::test]
async fn anonymize_succeeds_when_policies_supply_catalog_afresh() {
    let engine = engine();

    let policy = PolicyDefinition {
        id: uuid::Uuid::now_v7(),
        name: "test".into(),
        description: None,
        labels: Labels {
            builtins: vec![LabelRef::new("email_address")],
            custom: Vec::new(),
        },
        groups: Vec::new(),
        rules: Vec::new(),
        fallback: None,
    };
    let mut analyzed = engine
        .analyze(raw_docx(), std::slice::from_ref(&policy), &default_spec())
        .await
        .expect("analyze succeeds");

    engine
        .anonymize(raw_docx(), std::slice::from_ref(&policy), &mut analyzed)
        .await
        .expect("anonymize succeeds when catalog is re-derived from the same policy set");
}

#[tokio::test]
async fn audit_rejects_missing_context_on_deserialize() {
    let engine = engine();
    let analyzed = engine
        .analyze(raw_docx(), &[], &default_spec())
        .await
        .expect("analyze succeeds");
    let mut value = serde_json::to_value(&analyzed).expect("serialize");
    value
        .as_object_mut()
        .expect("object")
        .remove("context")
        .expect("context was serialized");

    let err = serde_json::from_value::<Audit>(value)
        .expect_err("deserializing without context must fail");
    assert!(
        err.to_string().contains("context"),
        "missing-field error must name `context`, got: {err}",
    );
}

#[tokio::test]
async fn analyze_rejects_policy_that_references_unknown_group() {
    use elide_governance::predicate::Predicate;
    use elide_governance::redaction::TextRedaction;
    use elide_governance::{PolicyRule, RuleDispatch};

    let engine = engine();
    let rule = PolicyRule {
        id: uuid::Uuid::now_v7(),
        name: "sweep".into(),
        description: None,
        dispatch: RuleDispatch::Predicated {
            predicate: Predicate::LabelInGroup {
                group: "definitely_no_such_group".to_owned(),
            },
            action: Box::new(ModalityRedactions {
                text: Some(TextRedaction::Erase),
                ..Default::default()
            }),
        },
    };
    let policy = PolicyDefinition {
        id: uuid::Uuid::now_v7(),
        name: "unknown-group".into(),
        description: None,
        labels: Labels::default(),
        groups: Vec::new(),
        rules: vec![rule],
        fallback: None,
    };

    let err = engine
        .analyze(raw_docx(), std::slice::from_ref(&policy), &default_spec())
        .await
        .expect_err("analyze must reject unknown group references");
    assert!(
        err.to_string().contains("definitely_no_such_group"),
        "error must name the unknown group, got: {err}",
    );
}

#[tokio::test]
async fn empty_analyzed_document_anonymize_fails_validation() {
    let engine = engine();
    let mut audit = Audit {
        body: None,
        parts: Default::default(),
        context: AuditContext {
            languages: Default::default(),
            countries: Vec::new(),
            metadata: Default::default(),
            correlation_id: uuid::Uuid::now_v7(),
            ocr_mode: Default::default(),
        },
    };
    let outcome = engine.anonymize(raw_docx(), &[], &mut audit).await;
    let err = outcome.expect_err("anonymize must reject a missing body group");
    assert!(
        err.to_string().contains("body group is missing"),
        "expected `body group is missing` error, got: {err}",
    );
}
