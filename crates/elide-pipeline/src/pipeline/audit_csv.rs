//! CSV export for [`Audit`]. Gated on the `audit-csv` cargo
//! feature.
//!
//! Three tables, one join key, via [`ExportCsv`]. Each writes to
//! any `impl Write`; callers decide where the bytes land (a file,
//! an HTTP body, an S3 upload):
//!
//! - [`Table::Entities`]: one row per detected entity. Scalar
//!   essentials only.
//! - [`Table::Provenance`]: one row per event on every entity's
//!   provenance chain.
//! - [`Table::Reviews`]: one row per reviewer decision.
//!
//! All three join on `entity_id`. The design choice for CSV is
//! honest scalar columns everywhere: no JSON blobs, no
//! polymorphic locations. Callers that need location details or
//! nested payloads use [`ExportJson`](elide_export::ExportJson).

use std::collections::HashMap;
use std::{io, result};

use elide::codec::PartId;
use elide::entity::Entity;
use elide::entity::audit::AuditKind;
use elide::modality::Modality;
use elide::modality::audio::Audio;
use elide::modality::image::Image;
use elide::modality::tabular::Tabular;
use elide::modality::text::Text;
use elide::{Error, ErrorKind, Report, Result};
use elide_export::{ExportCsv, Table, write_rows};
use elide_governance::modality::RedactableModality;
use serde::{Serialize, Serializer};
use uuid::Uuid;

use super::audit::Audit;
use crate::entity::Review;

/// Run `$body` once per modality, binding `$m` to the modality type
/// and `$name` to its wire name.
///
/// A report stores entities per modality and hands them back only
/// through a typed accessor, so every walk over "all entities" is
/// four probes. The modality list lives here so adding a fifth
/// means touching one macro rather than every row builder.
macro_rules! per_modality {
    (|$m:ident, $name:ident| $body:expr) => {{
        {
            type $m = Text;
            let $name = "text";
            $body
        }
        {
            type $m = Tabular;
            let $name = "tabular";
            $body
        }
        {
            type $m = Image;
            let $name = "image";
            $body
        }
        {
            type $m = Audio;
            let $name = "audio";
            $body
        }
    }};
}

impl ExportCsv for Audit {
    /// Entities first, then their provenance, then reviewer
    /// decisions: each table joins onto the previous one's
    /// `entity_id`, so this is the order a reader builds them up in.
    const TABLES: &'static [Table] = &[Table::Entities, Table::Provenance, Table::Reviews];

    /// Write one table of this audit as CSV.
    ///
    /// | table | columns |
    /// |---|---|
    /// | [`Entities`] | `part_id, modality, entity_id, label, confidence, coref` |
    /// | [`Provenance`] | `entity_id, event_index, kind, source, confidence, timestamp, payload_id` |
    /// | [`Reviews`] | `entity_id, modality, decision, operator` |
    ///
    /// Every table carries `entity_id` so they join back together.
    /// Rows are sorted for stable diffs: entities by
    /// `(part_id, entity_id)`, the others by `entity_id`.
    ///
    /// `part_id` is empty for body entities, `coref` is empty
    /// outside a coreference cluster, and `operator` is empty for a
    /// suppression, which names none.
    ///
    /// Locations and nested event payloads are dropped: CSV holds
    /// neither polymorphic locations nor event chains. Callers who
    /// need them use [`ExportJson`](elide_export::ExportJson).
    ///
    /// [`Entities`]: Table::Entities
    /// [`Provenance`]: Table::Provenance
    /// [`Reviews`]: Table::Reviews
    ///
    /// # Errors
    ///
    /// Returns [`ErrorKind::Processing`] on I/O or serialization
    /// failure.
    fn write_csv<W: io::Write>(&self, table: Table, writer: W) -> Result<()> {
        match table {
            Table::Entities => write_rows(writer, EntityRow::header(), self.entity_rows()),
            Table::Provenance => {
                write_rows(writer, ProvenanceRow::header(), self.provenance_rows())
            }
            Table::Reviews => write_rows(writer, ReviewRow::header(), self.review_rows()),
            // `Table` is `#[non_exhaustive]`: a table added there
            // that this audit cannot project is a caller error, not
            // a silent empty file.
            other => Err(Error::new(
                ErrorKind::Configuration,
                format!("audit has no `{other}` table to export"),
            )),
        }
    }
}

impl Audit {
    /// Iterate every entity across body + parts, sorted by
    /// `(part_id, entity_id)` for stable output.
    fn entity_rows(&self) -> Vec<EntityRow<'_>> {
        let mut rows = Vec::new();
        let parts = sorted_part_ids(&self.report);
        per_modality!(|M, name| {
            if let Some(entities) = self.report.entities::<M>() {
                extend_entity_rows(entities, None, name, &mut rows);
            }
            for id in &parts {
                if let Some(entities) = self.report.part_entities::<M>(id) {
                    extend_entity_rows(entities, Some(id.as_str()), name, &mut rows);
                }
            }
        });
        rows.sort_by(|a, b| {
            a.part_id
                .cmp(&b.part_id)
                .then(a.entity_id.cmp(&b.entity_id))
        });
        rows
    }

    /// One row per event on every entity's provenance chain,
    /// sorted by `(entity_id, event_index)`.
    fn provenance_rows(&self) -> Vec<ProvenanceRow<'_>> {
        let mut rows = Vec::new();
        let parts = sorted_part_ids(&self.report);
        per_modality!(|M, _name| {
            if let Some(entities) = self.report.entities::<M>() {
                extend_provenance_rows(entities, &mut rows);
            }
            for id in &parts {
                if let Some(entities) = self.report.part_entities::<M>(id) {
                    extend_provenance_rows(entities, &mut rows);
                }
            }
        });
        rows.sort_by(|a, b| {
            a.entity_id
                .cmp(&b.entity_id)
                .then(a.event_index.cmp(&b.event_index))
        });
        rows
    }

    /// One row per reviewer decision, sorted by `entity_id`.
    fn review_rows(&self) -> Vec<ReviewRow> {
        let mut rows: Vec<ReviewRow> = Vec::new();
        extend_review_rows(&self.reviews.text, "text", &mut rows);
        extend_review_rows(&self.reviews.tabular, "tabular", &mut rows);
        extend_review_rows(&self.reviews.image, "image", &mut rows);
        extend_review_rows(&self.reviews.audio, "audio", &mut rows);
        rows.sort_by_key(|row| row.entity_id);
        rows
    }
}

/// One entity row for CSV export. Every borrowed field points
/// into the source [`Audit`] for the lifetime of the export.
#[derive(Serialize)]
#[serde(rename_all = "snake_case")]
struct EntityRow<'a> {
    part_id: Option<&'a str>,
    modality: &'a str,
    entity_id: Uuid,
    label: &'a str,
    #[serde(serialize_with = "serialize_confidence")]
    confidence: f32,
    coref: Option<&'a str>,
}

/// One provenance row.
#[derive(Serialize)]
#[serde(rename_all = "snake_case")]
struct ProvenanceRow<'a> {
    entity_id: Uuid,
    event_index: usize,
    kind: &'static str,
    source: &'a str,
    #[serde(serialize_with = "serialize_confidence")]
    confidence: f32,
    timestamp: String,
    payload_id: Option<&'a str>,
}

fn extend_entity_rows<'a, M: Modality>(
    entities: &'a [Entity<M>],
    part_id: Option<&'a str>,
    modality: &'static str,
    out: &mut Vec<EntityRow<'a>>,
) {
    out.extend(entities.iter().map(|e| EntityRow {
        part_id,
        modality,
        entity_id: e.id,
        label: e.label.as_str(),
        confidence: f32::from(e.confidence),
        coref: e.coref.as_ref().map(|c| c.as_str()),
    }));
}

fn extend_provenance_rows<'a, M: Modality>(
    entities: &'a [Entity<M>],
    out: &mut Vec<ProvenanceRow<'a>>,
) {
    for entity in entities {
        for (index, event) in entity.audit.events().iter().enumerate() {
            let (kind, payload_id) = event_kind_and_payload(&event.kind);
            out.push(ProvenanceRow {
                entity_id: entity.id,
                event_index: index,
                kind,
                source: event.source.as_str(),
                confidence: f32::from(event.confidence),
                timestamp: event.timestamp.to_string(),
                payload_id,
            });
        }
    }
}

/// One row per decision in a modality's review bucket.
///
/// A suppression is a reviewer decision too, so it earns a row:
/// exporting only operator overrides would hide every "leave this
/// alone" call from the same report. It names no operator, so that
/// column stays empty — as does a retag, which corrects the
/// detection and lets the policy set pick again.
fn extend_review_rows<M: RedactableModality>(
    reviews: &HashMap<Uuid, Review<M>>,
    modality: &'static str,
    out: &mut Vec<ReviewRow>,
) {
    for (entity_id, review) in reviews {
        let decision = match review {
            Review::Redact { action, .. } => {
                let Some(operator) = operator_kind(action) else {
                    continue;
                };
                ("redact", operator)
            }
            Review::Suppress { .. } => ("suppress", String::new()),
            Review::Retag { .. } => ("retag", String::new()),
        };
        out.push(ReviewRow {
            entity_id: *entity_id,
            modality,
            decision: decision.0,
            operator: decision.1,
        });
    }
}

/// One reviewed entity, as the review CSV exports it.
///
/// `decision` and `operator` stay separate columns: a suppression
/// names no operator, so folding them would put a non-operator
/// value in a column consumers map onto an operator enum.
#[derive(Serialize)]
#[serde(rename_all = "snake_case")]
struct ReviewRow {
    entity_id: Uuid,
    modality: &'static str,
    decision: &'static str,
    operator: String,
}

impl EntityRow<'_> {
    const fn header() -> &'static [&'static str] {
        &[
            "part_id",
            "modality",
            "entity_id",
            "label",
            "confidence",
            "coref",
        ]
    }
}

impl ProvenanceRow<'_> {
    const fn header() -> &'static [&'static str] {
        &[
            "entity_id",
            "event_index",
            "kind",
            "source",
            "confidence",
            "timestamp",
            "payload_id",
        ]
    }
}

impl ReviewRow {
    const fn header() -> &'static [&'static str] {
        &["entity_id", "modality", "decision", "operator"]
    }
}

/// Discriminator + optional payload id for a provenance event
/// kind. Payload id carries the pattern rule name or model name
/// when the variant has one; empty otherwise.
fn event_kind_and_payload<M: Modality>(kind: &AuditKind<M>) -> (&'static str, Option<&str>) {
    match kind {
        AuditKind::Pattern(e) => ("pattern", Some(e.pattern.name.as_str())),
        AuditKind::Model(e) => ("model", Some(e.model.name.as_str())),
        AuditKind::Deduplication(e) => ("deduplication", Some(e.strategy.as_str())),
        AuditKind::Conflict(e) => ("conflict", Some(e.resolved_by.as_str())),
        AuditKind::Contested(e) => ("contested", Some(e.flagged_by.as_str())),
        AuditKind::Calibration(_) => ("calibration", None),
        AuditKind::Refinement(_) => ("refinement", None),
        AuditKind::Redaction(_) => ("redaction", None),
        AuditKind::Selection(e) => ("selection", Some(e.operator.name.as_str())),
        AuditKind::Manual(e) => ("manual", e.actor.as_deref()),
        // `AuditKind` is `#[non_exhaustive]`: a kind added upstream
        // lands here rather than breaking the build. Every variant
        // elide ships today is named above, so this arm firing means
        // a new one needs a column mapping.
        _ => ("unknown", None),
    }
}

/// Extract the operator `kind` discriminator from a review's
/// redaction spec. Uses serde JSON as a universal `kind` reader
/// across the four operator enums: each one is
/// `#[serde(tag = "kind")]`, so the top-level JSON object always
/// has a `"kind"` field.
fn operator_kind<R: Serialize>(action: &R) -> Option<String> {
    let value = serde_json::to_value(action).ok()?;
    value
        .get("kind")
        .and_then(|k| k.as_str())
        .map(|s| s.to_owned())
}

/// Format a `f32` confidence with three decimal places.
fn serialize_confidence<S: Serializer>(value: &f32, s: S) -> result::Result<S::Ok, S::Error> {
    s.serialize_str(&format!("{value:.3}"))
}

/// The report's container part ids, sorted, so an export is stable
/// across runs.
///
/// Borrowed from the report rather than cloned: the entity rows
/// carry `part_id` as a `&str` into it, so an owned vec here would
/// not outlive the rows that point at it.
fn sorted_part_ids(report: &Report) -> Vec<&PartId> {
    let mut ids: Vec<&PartId> = report.part_ids().map(|(id, _)| id).collect();
    ids.sort_by_key(|id| id.as_str());
    ids
}
