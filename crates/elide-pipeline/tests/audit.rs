//! End-to-end audit export tests over a small plaintext sample.
//!
//! Analyze `sample.txt`, exercise each `Audit::write_*` writer,
//! assert the output has the shape the module docs promise, and
//! drop the exports as `sample.audit.{json,entities.csv,...}`
//! artefacts under `tests/testdata/` for humans to inspect.
//! Reviewer-override paths are exercised by tagging one detected
//! entity with a `review` before the CSV export runs.

mod fixtures;

use std::io::{Cursor, Read};

use bytes::Bytes;
use elide::modality::text::Text;
use elide_export::{ExportCsv, ExportJson, Table};
use elide_pipeline::entity::{Edit, Reviewer, Suppress};
use elide_pipeline::file::Document;
use elide_pipeline::{Audit, Engine, ProviderConfig, RequestContext};

use self::fixtures::write_artefact;

const SAMPLE_TXT: &[u8] = include_bytes!("testdata/sample.txt");

fn raw_txt() -> Document {
    Document::new(Bytes::from_static(SAMPLE_TXT), "txt")
}

fn engine() -> Engine {
    Engine::new(ProviderConfig::default().build())
}

fn default_spec() -> RequestContext {
    RequestContext::new()
}

async fn analyze() -> Audit {
    engine()
        .analyze(raw_txt(), &[], &default_spec())
        .await
        .expect("analyze succeeds")
}

/// The body's text entity ids, in the order the recognizer emitted
/// them. Decisions key by id, so this is what a caller indexes.
fn text_entity_ids(audit: &Audit) -> Vec<uuid::Uuid> {
    audit
        .report
        .entities::<Text>()
        .expect("body present")
        .iter()
        .map(|entity| entity.id)
        .collect()
}

/// Tag the first detected entity with a text `Erase` review so
/// the review-export path has something to emit.
fn tag_first_with_review(audit: &mut Audit) -> uuid::Uuid {
    let ids = text_entity_ids(audit);
    assert!(!ids.is_empty(), "sample fixture must produce entities");
    audit.edit(Edit::<Text>::Suppress(Suppress {
        id: ids[0],
        by: Reviewer {
            reason: None,
            actor: None,
        },
    }));
    ids[0]
}

#[tokio::test]
async fn write_json_round_trips_via_serde_and_drops_artefact() {
    let audit = analyze().await;
    let mut buf = Vec::new();
    audit
        .write_json_pretty(&mut buf)
        .expect("write_json_pretty succeeds");
    write_artefact("sample", "audit.json", &buf);

    // `Audit` is Serialize-only: a serialized report names its
    // modalities but not their types, so the engine rebuilds it.
    let mut de = serde_json::Deserializer::from_slice(&buf);
    let round = engine()
        .deserialize_audit(&mut de)
        .expect("round-trip deserialize");
    let original = audit.report.entities::<Text>().expect("body is text");
    let round = round.report.entities::<Text>().expect("body is text");
    assert_eq!(
        original.len(),
        round.len(),
        "round-trip must preserve entity count",
    );
    assert_eq!(
        original[0].id, round[0].id,
        "round-trip must preserve entity ids",
    );
}

#[tokio::test]
async fn write_entities_csv_has_header_and_one_row_per_entity() {
    let audit = analyze().await;
    let mut buf = Vec::new();
    audit
        .write_csv(Table::Entities, &mut buf)
        .expect("entities table exports");
    write_artefact("sample", "audit-entities.csv", &buf);
    let output = String::from_utf8(buf).expect("csv is utf-8");

    let mut lines = output.lines();
    let header = lines.next().expect("header line present");
    assert_eq!(
        header, "part_id,modality,entity_id,label,confidence,coref",
        "column order matches the row struct",
    );

    let row_count = lines.count();
    let entities = audit.report.entities::<Text>().expect("body is text");
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
        .write_csv(Table::Provenance, &mut buf)
        .expect("provenance table exports");
    write_artefact("sample", "audit-provenance.csv", &buf);
    let output = String::from_utf8(buf).expect("csv is utf-8");

    let mut lines = output.lines();
    let header = lines.next().expect("header line present");
    assert_eq!(
        header,
        "entity_id,event_index,kind,source,confidence,timestamp,payload_id",
    );

    let row_count = lines.count();
    let entities = audit.report.entities::<Text>().expect("body is text");
    let expected_events: usize = entities
        .iter()
        .map(|entity| entity.audit.events().len())
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
        .write_csv(Table::Reviews, &mut buf)
        .expect("reviews table exports");
    write_artefact("sample", "audit-reviews.csv", &buf);
    let output = String::from_utf8(buf).expect("csv is utf-8");

    let mut lines = output.lines();
    let header = lines.next().expect("header line present");
    assert_eq!(header, "entity_id,modality,decision,operator,reason,actor");

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
        row.contains(",text,suppress,"),
        "modality and decision; a suppression names no operator, so \
         that column is empty; row: {row}",
    );
}

#[tokio::test]
async fn write_reviews_csv_lists_a_suppression_with_no_operator() {
    // A suppression is a reviewer decision too, so it earns a row.
    // It names no operator, so that column stays empty rather than
    // carrying a value that is not an operator kind.
    let mut audit = analyze().await;
    let suppressed_id = {
        let ids = text_entity_ids(&audit);
        assert!(!ids.is_empty(), "sample fixture must produce entities");
        audit.edit(Edit::<Text>::Suppress(Suppress {
            id: ids[0],
            by: Reviewer {
                reason: Some("false positive".into()),
                actor: Some("reviewer".into()),
            },
        }));
        ids[0]
    };

    let mut buf = Vec::new();
    audit
        .write_csv(Table::Reviews, &mut buf)
        .expect("reviews table exports");
    let output = String::from_utf8(buf).expect("csv is utf-8");

    let row = output
        .lines()
        .skip(1)
        .find(|row| row.contains(&suppressed_id.to_string()))
        .expect("the suppressed entity has a review row");
    assert!(
        row.contains(",text,suppress,"),
        "a suppression exports its decision and an empty operator; row: {row}",
    );
}

#[tokio::test]
async fn write_reviews_csv_writes_header_when_no_reviews_set() {
    let audit = analyze().await;
    let mut buf = Vec::new();
    audit
        .write_csv(Table::Reviews, &mut buf)
        .expect("reviews table exports");
    let output = String::from_utf8(buf).expect("csv is utf-8");

    let line_count = output.lines().count();
    assert_eq!(
        line_count, 1,
        "header must be written even when there are no data rows",
    );
    assert_eq!(
        output.lines().next().unwrap(),
        "entity_id,modality,decision,operator,reason,actor",
    );
}

#[tokio::test]
async fn export_csv_writes_every_table_by_iterating_tables() {
    // What the trait buys over three inherent methods: a caller
    // exports the whole audit without naming each table, and gains
    // any table added later for free.
    let mut audit = analyze().await;
    tag_first_with_review(&mut audit);

    for table in <Audit as ExportCsv>::TABLES {
        let mut buf = Vec::new();
        audit
            .write_csv(*table, &mut buf)
            .unwrap_or_else(|err| panic!("{table} table exports: {err}"));
        let output = String::from_utf8(buf).expect("csv is utf-8");
        let header = output.lines().next().expect("header line present");
        assert!(
            header.contains(Table::JOIN_KEY),
            "every table carries the join key so they recombine; \
             {table} header was {header:?}",
        );
    }
}

#[tokio::test]
async fn export_csv_bundles_every_table_into_one_zip() {
    let mut audit = analyze().await;
    tag_first_with_review(&mut audit);

    let archive = audit.to_zip().expect("audit bundles into a zip");
    write_artefact("sample", "audit-tables.zip", &archive);

    let mut zip = zip::ZipArchive::new(Cursor::new(archive)).expect("archive opens");
    let names: Vec<String> = zip.file_names().map(ToOwned::to_owned).collect();
    for table in <Audit as ExportCsv>::TABLES {
        assert!(
            names.contains(&format!("{table}.csv")),
            "the archive carries one entry per table; got {names:?}",
        );
    }

    // Entries are real CSV, not empty placeholders.
    let mut entry = zip
        .by_name("entities.csv")
        .expect("the entities table is in the archive");
    let mut csv = String::new();
    entry.read_to_string(&mut csv).expect("entry is utf-8");
    assert!(
        csv.lines()
            .next()
            .is_some_and(|h| h.contains(Table::JOIN_KEY)),
        "the archived table keeps its header; got {csv:?}",
    );
}
