//! End-to-end at the engine layer over a DOCX (text body +
//! embedded PNG): analyze, override one entity, anonymize,
//! assert the body changed and the image part round-tripped
//! unchanged.

mod fixtures;

use std::io::Read;

use bytes::Bytes;
use nvisy_engine::Engine;
use nvisy_engine::findings::{Findings, RecognizedGroup};
use nvisy_schema::file::Document;
use nvisy_schema::plan::{
    AnalyzerParams, OcrBackendParams, OcrEnricherParams, PatternRecognizerParams, ScopeParams,
};
use nvisy_schema::policy::PolicyAction;
use nvisy_schema::policy::redaction::{ModalityRedactions, TextRedaction};

use self::fixtures::write_artefact;

const SAMPLE_DOCX: &[u8] = include_bytes!("testdata/sample.docx");
const IMAGE_PART_ID: &str = "word/media/image1.png";

fn raw_docx() -> Document {
    Document::new(Bytes::from_static(SAMPLE_DOCX), "docx")
}

fn engine() -> Engine {
    Engine::new()
}

fn default_spec() -> AnalyzerParams {
    AnalyzerParams {
        recognizers: nvisy_schema::plan::RecognizerParams {
            pattern: Some(PatternRecognizerParams {
                builtins: true,
                context_enhanced: true,
            }),
            ner: false,
            llm: false,
        },
        enrichers: nvisy_schema::plan::EnricherParams {
            language: None,
            ocr: Some(OcrEnricherParams {
                backend: OcrBackendParams::Mock,
            }),
            stt: None,
        },
        deduplication: Default::default(),
        scope: ScopeParams::default(),
    }
}

fn read_zip_entry(buf: &[u8], name: &str) -> Option<Vec<u8>> {
    let mut zip = zip::ZipArchive::new(std::io::Cursor::new(buf.to_vec())).ok()?;
    let mut entry = zip.by_name(name).ok()?;
    let mut out = Vec::new();
    entry.read_to_end(&mut out).ok()?;
    Some(out)
}

#[tokio::test]
async fn analyze_captures_text_body_and_image_part() {
    let engine = engine();
    let findings = engine
        .analyze_document(raw_docx(), &default_spec(), &[])
        .await
        .expect("analyze succeeds");

    let body_group = findings.body.as_ref().expect("body group present");
    let text_entities = match body_group {
        RecognizedGroup::Text { entities } => entities,
        other => panic!("expected Text body, got {other:?}"),
    };
    assert!(
        !text_entities.is_empty(),
        "fixture should carry at least one body entity",
    );

    let image_group = findings.parts.get(IMAGE_PART_ID).unwrap_or_else(|| {
        panic!(
            "expected part `{IMAGE_PART_ID}` in findings.parts; got keys: {:?}",
            findings.parts.keys().collect::<Vec<_>>(),
        )
    });
    assert!(
        matches!(image_group, RecognizedGroup::Image { .. }),
        "expected Image variant for image part, got {image_group:?}",
    );
}

#[tokio::test]
async fn anonymize_redacts_targeted_entity_and_preserves_other_parts() {
    let engine = engine();
    let mut findings = engine
        .analyze_document(raw_docx(), &default_spec(), &[])
        .await
        .expect("analyze succeeds");
    let body_group = findings.body.as_ref().expect("body group present");
    let RecognizedGroup::Text { entities } = body_group else {
        panic!("expected Text body");
    };
    assert!(
        !entities.is_empty(),
        "fixture should carry at least one entity"
    );

    let target_id = entities[0].entity.id;
    let RecognizedGroup::Text { entities: muts } = findings.body.as_mut().unwrap() else {
        unreachable!()
    };
    muts.iter_mut()
        .find(|r| r.entity.id == target_id)
        .expect("entity present")
        .r#override = Some(PolicyAction::Redact(ModalityRedactions {
        text: Some(TextRedaction::Erase),
        tabular: None,
        image: None,
        audio: None,
    }));

    let outcome = engine
        .anonymize_document(raw_docx(), &default_spec(), &[], &findings)
        .await
        .expect("anonymize succeeds");
    write_artefact("sample", "docx", &outcome.bytes);

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
async fn empty_findings_anonymize_fails_validation() {
    let engine = engine();
    let outcome = engine
        .anonymize_document(raw_docx(), &default_spec(), &[], &Findings::default())
        .await;
    let err = outcome.expect_err("anonymize must reject a missing body group");
    assert!(
        err.to_string().contains("body group is missing"),
        "expected `body group is missing` error, got: {err}",
    );
}
