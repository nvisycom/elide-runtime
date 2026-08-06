//! End-to-end audit export tests over a small plaintext sample.
//!
//! Analyze `sample.txt`, exercise each `Audit::write_*` writer,
//! assert the output has the shape the module docs promise, and
//! drop the exports as `sample.audit.{json,entities.csv,...}`
//! artefacts under `tests/testdata/` for humans to inspect.
//! Reviewer-override paths are exercised by tagging one detected
//! entity with a `review` before the CSV export runs.

mod fixtures;

use bytes::Bytes;
use nvisy_engine::entity::EntityRecord;
use nvisy_engine::modality::Text;
use nvisy_engine::{Audit, Engine, RecognizedGroup};
use nvisy_schema::file::Document;
use nvisy_schema::plan::{
    AnalyzerParams, EnricherParams, PatternRecognizerParams, ProviderSelection, RecognizerParams,
    ScopeParams,
};
use nvisy_schema::policy::redaction::{ModalityRedactions, TextRedaction};

use self::fixtures::write_artefact;

const SAMPLE_TXT: &[u8] = include_bytes!("testdata/sample.txt");

fn raw_txt() -> Document {
    Document::new(Bytes::from_static(SAMPLE_TXT), "txt")
}

fn engine() -> Engine {
    Engine::new()
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

async fn analyze() -> Audit {
    engine()
        .analyze_document(raw_txt(), &[], &default_spec())
        .await
        .expect("analyze succeeds")
}

fn text_records_mut(audit: &mut Audit) -> &mut Vec<EntityRecord<Text>> {
    let RecognizedGroup::Text { entities } = audit.body.as_mut().expect("body present") else {
        panic!("expected text body");
    };
    entities
}

/// Tag the first detected entity with a text `Erase` review so
/// the review-export path has something to emit.
fn tag_first_with_review(audit: &mut Audit) -> uuid::Uuid {
    let records = text_records_mut(audit);
    assert!(!records.is_empty(), "sample fixture must produce entities");
    let target = records[0].entity.id;
    records[0].review = Some(ModalityRedactions {
        text: Some(TextRedaction::Erase),
        ..Default::default()
    });
    target
}

#[tokio::test]
async fn write_json_round_trips_via_serde_and_drops_artefact() {
    let audit = analyze().await;
    let mut buf = Vec::new();
    audit.write_json(&mut buf).expect("write_json succeeds");
    write_artefact("sample", "audit.json", &buf);

    let round: Audit = serde_json::from_slice(&buf).expect("round-trip deserialize");
    let RecognizedGroup::Text { entities: original } = audit.body.as_ref().unwrap() else {
        panic!("original body is not text");
    };
    let RecognizedGroup::Text { entities: round } = round.body.as_ref().unwrap() else {
        panic!("round-tripped body is not text");
    };
    assert_eq!(
        original.len(),
        round.len(),
        "round-trip must preserve entity count",
    );
    assert_eq!(
        original[0].entity.id, round[0].entity.id,
        "round-trip must preserve entity ids",
    );
}

#[tokio::test]
async fn write_entities_csv_has_header_and_one_row_per_entity() {
    let audit = analyze().await;
    let mut buf = Vec::new();
    audit
        .write_entities_csv(&mut buf)
        .expect("write_entities_csv succeeds");
    write_artefact("sample", "audit-entities.csv", &buf);
    let output = String::from_utf8(buf).expect("csv is utf-8");

    let mut lines = output.lines();
    let header = lines.next().expect("header line present");
    assert_eq!(
        header,
        "part_id,modality,entity_id,label,confidence,coref",
        "column order matches the row struct",
    );

    let row_count = lines.count();
    let RecognizedGroup::Text { entities } = audit.body.as_ref().unwrap() else {
        unreachable!()
    };
    assert_eq!(
        row_count,
        entities.len(),
        "one row per entity (body only, no parts in this fixture)",
    );
}

#[tokio::test]
async fn write_provenance_csv_emits_one_row_per_event() {
    let audit = analyze().await;
    let mut buf = Vec::new();
    audit
        .write_provenance_csv(&mut buf)
        .expect("write_provenance_csv succeeds");
    write_artefact("sample", "audit-provenance.csv", &buf);
    let output = String::from_utf8(buf).expect("csv is utf-8");

    let mut lines = output.lines();
    let header = lines.next().expect("header line present");
    assert_eq!(
        header,
        "entity_id,event_index,kind,source,before,after,at,payload_id",
    );

    let row_count = lines.count();
    let RecognizedGroup::Text { entities } = audit.body.as_ref().unwrap() else {
        unreachable!()
    };
    let expected_events: usize = entities
        .iter()
        .map(|r| r.entity.provenance.events.len())
        .sum();
    assert_eq!(
        row_count, expected_events,
        "one row per event across the whole audit",
    );
}

#[tokio::test]
async fn write_reviews_csv_only_lists_reviewed_entities() {
    let mut audit = analyze().await;
    let reviewed_id = tag_first_with_review(&mut audit);

    let mut buf = Vec::new();
    audit
        .write_reviews_csv(&mut buf)
        .expect("write_reviews_csv succeeds");
    write_artefact("sample", "audit-reviews.csv", &buf);
    let output = String::from_utf8(buf).expect("csv is utf-8");

    let mut lines = output.lines();
    let header = lines.next().expect("header line present");
    assert_eq!(header, "entity_id,modality,operator");

    let rows: Vec<&str> = lines.collect();
    assert_eq!(
        rows.len(),
        1,
        "only the one reviewed entity should appear; got {rows:?}",
    );
    let row = rows[0];
    assert!(
        row.contains(&reviewed_id.to_string()),
        "review row must carry the reviewed entity's id; row: {row}",
    );
    assert!(
        row.ends_with(",text,erase"),
        "modality + operator kind extracted from the text redaction; row: {row}",
    );
}

#[tokio::test]
async fn write_reviews_csv_writes_header_when_no_reviews_set() {
    let audit = analyze().await;
    let mut buf = Vec::new();
    audit
        .write_reviews_csv(&mut buf)
        .expect("write_reviews_csv succeeds");
    let output = String::from_utf8(buf).expect("csv is utf-8");

    let line_count = output.lines().count();
    assert_eq!(
        line_count, 1,
        "header must be written even when there are no data rows",
    );
    assert_eq!(
        output.lines().next().unwrap(),
        "entity_id,modality,operator",
    );
}
