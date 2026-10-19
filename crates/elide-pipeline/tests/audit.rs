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
use elide::entity::LabelRef;
use elide::modality::text::Text;
use elide_export::{ExportCsv, ExportJson, Table};
use elide_pipeline::entity::{Edit, EditSet, Reviewer, Suppress};
use elide_pipeline::file::Document;
use elide_pipeline::provider::{Deployment, Provider, RequestContext};
use elide_pipeline::{Audit, Engine};
use elide_schema::policy::{LabelScope, Policy};

use self::fixtures::write_artefact;

const SAMPLE_TXT: &[u8] = include_bytes!("testdata/sample.txt");

fn raw_txt() -> Document {
    Document::new("sample.txt", Bytes::from_static(SAMPLE_TXT))
}

fn engine() -> Engine {
    Engine::new(Provider::new(Deployment::default()))
}

fn default_spec() -> RequestContext {
    RequestContext::new()
}

/// Detect the sample's contact labels without redacting them: a
/// request names the labels to find, and these tests exercise the
/// export writers over what detection produced.
fn detect_only() -> Policy {
    Policy {
        id: uuid::Uuid::now_v7(),
        name: "detect-contacts".into(),
        scopes: vec![LabelScope::new(
            "contact",
            vec![
                LabelRef::new("email_address"),
                LabelRef::new("phone_number"),
            ],
        )],
        ..Policy::default()
    }
}

async fn analyze() -> Audit {
    engine()
        .analyze(raw_txt(), &[detect_only()], &default_spec())
        .await
        .expect("analyze succeeds")
        .audit
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

/// Suppress the first detected entity as a reviewer would, so the
/// review-export path has something to emit.
///
/// The edits are *applied*: an `EditSet` on its own leaves no trace
/// on the report, and it is the `Manual` event `apply` stamps that
/// the reviews table projects. Before this applied them, the two
/// tests below asserted over an audit no reviewer had touched.
fn suppress_first_as_reviewer(audit: &mut Audit, reason: Option<&str>) -> uuid::Uuid {
    let ids = text_entity_ids(audit);
    assert!(!ids.is_empty(), "sample fixture must produce entities");
    let mut edits = EditSet::default();
    edits.edit(Edit::<Text>::Suppress(Suppress {
        id: ids[0],
        by: Reviewer {
            reason: reason.map(Into::into),
            actor: Some("reviewer@example.com".into()),
        },
    }));
    edits
        .apply(&mut audit.report)
        .expect("the edit names an entity the report holds");
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
        "one row per entity: the fixture is one document with no \
         nested parts, and the sole-document shorthand must not \
         double-count it against the part tree",
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
async fn export_csv_writes_every_table_by_iterating_tables() {
    // What the trait buys over three inherent methods: a caller
    // exports the whole audit without naming each table, and gains
    // any table added later for free.
    let mut audit = analyze().await;
    suppress_first_as_reviewer(&mut audit, None);

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
    suppress_first_as_reviewer(&mut audit, None);

    let archive = audit.to_zip().expect("audit bundles into a zip");
    write_artefact("sample", "audit-tables.zip", &archive);

    let mut zip = zip::ZipArchive::new(Cursor::new(archive)).expect("archive opens");
    // zip 9 yields `Result<Cow<str>>` per entry rather than `&str`:
    // a name can be invalid UTF-8, which it now reports instead of
    // hiding.
    let names: Vec<String> = zip
        .file_names()
        .map(|name| {
            name.expect("fixture entry names are valid UTF-8")
                .into_owned()
        })
        .collect();
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

#[tokio::test]
async fn write_reviews_csv_emits_one_row_per_override_with_its_reason() {
    let mut audit = analyze().await;
    let suppressed = suppress_first_as_reviewer(&mut audit, Some("false positive: a sample name"));

    let mut buf = Vec::new();
    audit
        .write_csv(Table::Reviews, &mut buf)
        .expect("reviews table exports");
    write_artefact("sample", "audit-reviews.csv", &buf);
    let output = String::from_utf8(buf).expect("csv is utf-8");

    let mut lines = output.lines();
    assert_eq!(
        lines.next().expect("header line present"),
        "entity_id,intent,reviewer,authority,reason,timestamp",
    );

    let rows: Vec<&str> = lines.collect();
    assert_eq!(rows.len(), 1, "one override was made; got {rows:?}");
    let row = rows[0];

    assert!(
        row.contains(&suppressed.to_string()),
        "the row must name the entity overridden, so it joins the other tables: {row}",
    );
    assert!(
        row.contains("suppress"),
        "and which decision was made: {row}",
    );
    assert!(
        row.contains("false positive: a sample name"),
        "and the reviewer's reason, which is the whole point of this table — \
         the provenance table renders the intent but has no column for it: {row}",
    );
    assert!(
        row.contains("reviewer@example.com"),
        "and who made it: {row}",
    );
}

#[tokio::test]
async fn an_unreviewed_audit_exports_an_empty_reviews_table() {
    // An export runs whether or not a review happened. A header
    // with no rows says "nothing was overridden"; failing would
    // make the common case look like an error.
    let audit = analyze().await;

    let mut buf = Vec::new();
    audit
        .write_csv(Table::Reviews, &mut buf)
        .expect("the table exports even with no overrides");
    let output = String::from_utf8(buf).expect("csv is utf-8");

    assert_eq!(
        output.lines().count(),
        1,
        "header only, no rows: {output:?}",
    );
}
