//! End-to-end at the engine layer over a DOCX (text body +
//! embedded PNG): analyze, override one entity, anonymize,
//! assert the body changed and the image part round-tripped
//! unchanged.

mod fixtures;

use std::io::{Cursor, Read};

use bytes::Bytes;
use elide::entity::LabelRef;
use elide::modality::image::Image;
use elide::modality::text::Text;
use elide::{ErrorKind, PartId, Report};
use elide_governance::redaction::ModalityRedactions;
use elide_governance::{LabelScope, PolicyDefinition};
use elide_pipeline::file::Document;
use elide_pipeline::{
    Audit, CodecParams, Component, DocumentContext, Engine, Enrichers, OcrBackend, ProviderConfig,
    RequestContext,
};

use self::fixtures::write_artefact;

const SAMPLE_DOCX: &[u8] = include_bytes!("testdata/sample.docx");
const IMAGE_PART_ID: &str = "word/media/image1.png";
/// The name the fixture is analyzed under: since elide unified the
/// body into the part tree, it roots every part path in the report.
const DOCUMENT: &str = "sample.docx";

fn raw_docx() -> Document {
    Document::new(DOCUMENT, Bytes::from_static(SAMPLE_DOCX))
}

fn engine() -> Engine {
    Engine::new(
        ProviderConfig {
            enrichers: Enrichers {
                ocr: vec![Component::<OcrBackend> {
                    name: "mock".into(),
                    description: None,
                    backend: OcrBackend::Mock,
                }],
                ..Enrichers::default()
            },
            ..ProviderConfig::default()
        }
        .build(),
    )
}

fn default_spec() -> RequestContext {
    RequestContext::new()
}

/// Detect the fixture's contact labels without redacting them.
/// A request names the labels to find; these tests assert on what
/// detection produced and on part round-tripping.
fn detect_only() -> PolicyDefinition {
    PolicyDefinition {
        id: uuid::Uuid::now_v7(),
        name: "detect-contacts".into(),
        description: None,
        template: None,
        scopes: vec![LabelScope::new(
            "contact",
            vec![
                LabelRef::new("email_address"),
                LabelRef::new("phone_number"),
            ],
        )],
        custom: Vec::new(),
        rules: Vec::new(),
        fallback: None,
    }
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
        .analyze(raw_docx(), &[detect_only()], &default_spec())
        .await
        .expect("analyze succeeds")
        .audit;

    let text_entities = analyzed
        .report
        .entities::<Text>()
        .expect("expected a Text body");
    assert!(
        !text_entities.is_empty(),
        "fixture should carry at least one body entity",
    );

    let part_id = PartId::new(DOCUMENT).child(IMAGE_PART_ID);
    assert!(
        analyzed.report.part_entities::<Image>(&part_id).is_some(),
        "expected part `{IMAGE_PART_ID}` to carry Image entities; got parts: {:?}",
        analyzed
            .report
            .part_ids()
            .map(|(id, _)| id.to_string())
            .collect::<Vec<_>>(),
    );
}

#[tokio::test]
async fn anonymize_redacts_targeted_entity_and_preserves_other_parts() {
    let engine = engine();
    let mut analyzed = engine
        .analyze(raw_docx(), &[detect_only()], &default_spec())
        .await
        .expect("analyze succeeds")
        .audit;
    let entities = analyzed
        .report
        .entities::<Text>()
        .expect("expected a Text body");
    assert!(
        !entities.is_empty(),
        "fixture should carry at least one entity"
    );
    // A policy that erases everything it sees, so the body is
    // guaranteed to change while the image part is left alone.
    let review_policy = PolicyDefinition {
        id: uuid::Uuid::now_v7(),
        name: "erase-everything".into(),
        description: None,
        template: None,
        scopes: vec![LabelScope::new(
            "contact",
            vec![LabelRef::new("email_address")],
        )],
        custom: Vec::new(),
        rules: Vec::new(),
        fallback: Some(ModalityRedactions {
            text: Some(elide_governance::redaction::TextRedaction::Erase),
            ..Default::default()
        }),
    };

    let outcome = engine
        .anonymize(
            raw_docx(),
            std::slice::from_ref(&review_policy),
            &mut analyzed,
            None,
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
        "the policy's erase must change the body XML",
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
    spec.context.metadata.tags = vec!["gdpr-request".into()];
    spec.context.metadata.purpose = Some("dsar-response".into());
    spec.context.metadata.audience = vec!["data-subject".into(), "compliance-review".into()];
    let doc = raw_docx();

    let audit = engine
        .analyze(doc, &[detect_only()], &spec)
        .await
        .expect("analyze succeeds")
        .audit;

    assert_eq!(
        audit.context.metadata.tags, spec.context.metadata.tags,
        "the audit must carry back the caller-asserted scope tags",
    );
    assert_eq!(
        audit.context.metadata.purpose, spec.context.metadata.purpose,
        "the audit must carry back the caller-asserted scope purpose",
    );
    assert_eq!(
        audit.context.metadata.audience, spec.context.metadata.audience,
        "the audit must carry back the caller-asserted scope audience",
    );
}

#[tokio::test]
async fn anonymize_succeeds_when_policies_supply_catalog_afresh() {
    let engine = engine();

    let policy = detect_only();
    let mut analyzed = engine
        .analyze(raw_docx(), std::slice::from_ref(&policy), &default_spec())
        .await
        .expect("analyze succeeds")
        .audit;

    engine
        .anonymize(
            raw_docx(),
            std::slice::from_ref(&policy),
            &mut analyzed,
            None,
        )
        .await
        .expect("anonymize succeeds when catalog is re-derived from the same policy set");
}

#[tokio::test]
async fn audit_rejects_a_missing_round_trip_field_on_deserialize() {
    let engine = engine();
    let analyzed = engine
        .analyze(raw_docx(), &[detect_only()], &default_spec())
        .await
        .expect("analyze succeeds")
        .audit;
    let serialized = serde_json::to_value(&analyzed).expect("serialize");

    // Both are always serialized, and both pin something anonymize
    // must not re-derive: the recognition vocabulary, and how the
    // document decodes. A payload missing either is malformed, not
    // empty — defaulting `codec` would silently re-decode a PDF
    // differently than analyze did, landing entity offsets on
    // different content.
    for field in ["context", "codec"] {
        let mut value = serialized.clone();
        value
            .as_object_mut()
            .expect("object")
            .remove(field)
            .unwrap_or_else(|| panic!("`{field}` was serialized"));

        // Deserialization runs through the engine, which holds the
        // modality registry a serialized report needs to be rebuilt.
        let json = serde_json::to_string(&value).expect("re-serialize");
        let mut de = serde_json::Deserializer::from_str(&json);
        let Err(err) = engine.deserialize_audit(&mut de) else {
            panic!("deserializing without `{field}` must fail");
        };
        assert!(
            err.to_string().contains(field),
            "missing-field error must name `{field}`, got: {err}",
        );
    }
}

#[tokio::test]
async fn analyze_rejects_policy_that_references_unknown_group() {
    use elide_governance::redaction::TextRedaction;
    use elide_governance::{PolicyRule, Predicate, RuleDispatch};

    let engine = engine();
    let rule = PolicyRule {
        id: uuid::Uuid::now_v7(),
        name: "sweep".into(),
        description: None,
        attribution: None,
        dispatch: RuleDispatch::Predicated {
            predicate: Predicate::LabelInScope {
                scope: "definitely_no_such_group".to_owned(),
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
        template: None,
        scopes: Vec::new(),
        custom: Vec::new(),
        rules: vec![rule],
        fallback: None,
    };

    // `Audit` is not `Debug` (it holds an elide `Report`), so the
    // error is matched out rather than `expect_err`ed.
    let Err(err) = engine
        .analyze(raw_docx(), std::slice::from_ref(&policy), &default_spec())
        .await
    else {
        panic!("analyze must reject unknown group references");
    };
    assert!(
        err.to_string().contains("definitely_no_such_group"),
        "error must name the unknown group, got: {err}",
    );
}

#[tokio::test]
async fn anonymize_rejects_an_audit_that_never_ran_analyze() {
    // An audit with no body was never analyzed. Applying it would
    // hand back the document unredacted and report success, which a
    // caller cannot tell from "there was nothing to redact" — so it
    // is refused instead.
    let engine = engine();
    let mut audit = Audit {
        report: Report::new(),
        context: DocumentContext::default(),
        codec: CodecParams::default(),
        usage: Default::default(),
    };

    let Err(err) = engine
        .anonymize(raw_docx(), &[detect_only()], &mut audit, None)
        .await
    else {
        panic!("an audit with no body must not silently redact nothing");
    };
    assert_eq!(err.kind(), ErrorKind::Configuration, "{err}");
    assert!(
        err.to_string().contains("analyze must run first"),
        "the error names the missing step; got: {err}",
    );
}

/// Artifacts ride beside the audit, not on it: an image body's OCR
/// enrichment reaches the caller through [`Analyzed::artifacts`],
/// survives a JSON round trip through `Engine::deserialize_artifacts`,
/// and seeds `re_analyze` — while the serialized audit itself carries
/// no document content.
#[tokio::test]
async fn artifacts_round_trip_beside_the_audit_and_seed_a_re_run() {
    let engine = engine();
    let analyzed = engine
        .analyze(raw_docx(), &[detect_only()], &default_spec())
        .await
        .expect("analyze succeeds");

    // The audit serializes without the enrichment: an audit is
    // references and decisions, so persisting one cannot leak the
    // document's content.
    let audit_json = serde_json::to_string(&analyzed.audit).expect("audit serializes");
    assert!(
        !audit_json.contains("artifacts"),
        "the audit must not carry enrichment content",
    );

    // The artifacts serialize separately and rebuild through the engine.
    let json = serde_json::to_string(&analyzed.artifacts).expect("artifacts serialize");
    let mut de = serde_json::Deserializer::from_str(&json);
    let restored = engine
        .deserialize_artifacts(&mut de)
        .expect("artifacts rebuild from the wire");

    // The restored set seeds a re-run, which detects as the first pass did.
    let again = engine
        .re_analyze(raw_docx(), &[detect_only()], &default_spec(), &restored)
        .await
        .expect("re_analyze succeeds");

    let first = analyzed
        .audit
        .report
        .entities::<Text>()
        .expect("expected a Text body")
        .len();
    let second = again
        .audit
        .report
        .entities::<Text>()
        .expect("expected a Text body")
        .len();
    assert_eq!(
        first, second,
        "a seeded re-run finds what the first pass found",
    );
}
