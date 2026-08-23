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

use std::{io, result};

use elide::entity::audit::AuditKind;
use elide::modality::Modality;
use elide::{Error, ErrorKind, Result};
use elide_export::{ExportCsv, Table, write_rows};
use elide_governance::modality::RedactableModality;
use serde::{Serialize, Serializer};
use uuid::Uuid;

use super::audit::Audit;
use crate::entity::{EntityGroup, EntityRecord, Review};

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
        let mut rows: Vec<EntityRow<'_>> = Vec::new();
        if let Some(group) = &self.body {
            push_entity_rows(group, None, &mut rows);
        }
        let mut part_keys: Vec<&String> = self.parts.keys().collect();
        part_keys.sort();
        for id in part_keys {
            push_entity_rows(&self.parts[id], Some(id.as_str()), &mut rows);
        }
        rows
    }

    /// Iterate every provenance event across body + parts,
    /// sorted by `(entity_id, event_index)`.
    fn provenance_rows(&self) -> Vec<ProvenanceRow<'_>> {
        let mut rows: Vec<ProvenanceRow<'_>> = Vec::new();
        if let Some(group) = &self.body {
            push_provenance_rows(group, &mut rows);
        }
        let mut part_keys: Vec<&String> = self.parts.keys().collect();
        part_keys.sort();
        for id in part_keys {
            push_provenance_rows(&self.parts[id], &mut rows);
        }
        rows.sort_by(|a, b| {
            a.entity_id
                .cmp(&b.entity_id)
                .then_with(|| a.event_index.cmp(&b.event_index))
        });
        rows
    }

    /// Iterate every reviewed entity across body + parts.
    fn review_rows(&self) -> Vec<ReviewRow> {
        let mut rows: Vec<ReviewRow> = Vec::new();
        if let Some(group) = &self.body {
            push_review_rows(group, &mut rows);
        }
        let mut part_keys: Vec<&String> = self.parts.keys().collect();
        part_keys.sort();
        for id in part_keys {
            push_review_rows(&self.parts[id], &mut rows);
        }
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

fn push_entity_rows<'a>(
    group: &'a EntityGroup,
    part_id: Option<&'a str>,
    out: &mut Vec<EntityRow<'a>>,
) {
    match group {
        EntityGroup::Text(entities) => extend_entity_rows(entities, part_id, "text", out),
        EntityGroup::Tabular(entities) => extend_entity_rows(entities, part_id, "tabular", out),
        EntityGroup::Image(entities) => extend_entity_rows(entities, part_id, "image", out),
        EntityGroup::Audio(entities) => extend_entity_rows(entities, part_id, "audio", out),
    }
}

fn push_provenance_rows<'a>(group: &'a EntityGroup, out: &mut Vec<ProvenanceRow<'a>>) {
    match group {
        EntityGroup::Text(entities) => extend_provenance_rows(entities, out),
        EntityGroup::Tabular(entities) => extend_provenance_rows(entities, out),
        EntityGroup::Image(entities) => extend_provenance_rows(entities, out),
        EntityGroup::Audio(entities) => extend_provenance_rows(entities, out),
    }
}

fn push_review_rows(group: &EntityGroup, out: &mut Vec<ReviewRow>) {
    match group {
        EntityGroup::Text(entities) => extend_review_rows(entities, "text", out),
        EntityGroup::Tabular(entities) => extend_review_rows(entities, "tabular", out),
        EntityGroup::Image(entities) => extend_review_rows(entities, "image", out),
        EntityGroup::Audio(entities) => extend_review_rows(entities, "audio", out),
    }
}

fn extend_entity_rows<'a, M: RedactableModality>(
    records: &'a [EntityRecord<M>],
    part_id: Option<&'a str>,
    modality: &'static str,
    out: &mut Vec<EntityRow<'a>>,
) {
    let start = out.len();
    for r in records {
        out.push(EntityRow {
            part_id,
            modality,
            entity_id: r.entity.id,
            label: r.entity.label.as_str(),
            confidence: f32::from(r.entity.confidence),
            coref: r.entity.coref.as_ref().map(|c| c.as_str()),
        });
    }
    // Stable sort by entity_id within this group so the
    // (part_id, entity_id) global ordering holds after the
    // caller concatenates groups in part-id order.
    out[start..].sort_by_key(|r| r.entity_id);
}

fn extend_provenance_rows<'a, M: RedactableModality>(
    records: &'a [EntityRecord<M>],
    out: &mut Vec<ProvenanceRow<'a>>,
) {
    for r in records {
        for (i, event) in r.entity.audit.events().iter().enumerate() {
            let (kind, payload_id) = event_kind_and_payload(&event.kind);
            out.push(ProvenanceRow {
                entity_id: r.entity.id,
                event_index: i,
                kind,
                source: event.source.as_str(),
                confidence: f32::from(event.confidence),
                timestamp: event.timestamp.to_string(),
                payload_id,
            });
        }
    }
}

fn extend_review_rows<M: RedactableModality>(
    records: &[EntityRecord<M>],
    modality: &'static str,
    out: &mut Vec<ReviewRow>,
) {
    for r in records {
        // A suppression is a reviewer decision too, so it earns a
        // row: exporting only operator overrides would hide every
        // "leave this alone" call from the same report. It names no
        // operator, so that column stays empty.
        let row = match &r.review {
            Some(Review::Redact { action, .. }) => {
                operator_kind(action).map(|operator| ("redact", operator))
            }
            Some(Review::Suppress { .. }) => Some(("suppress", String::new())),
            // A retag names no operator either: it corrects the
            // detection and lets the policy set pick again.
            Some(Review::Retag { .. }) => Some(("retag", String::new())),
            None => None,
        };
        if let Some((decision, operator)) = row {
            out.push(ReviewRow {
                entity_id: r.entity.id,
                modality,
                decision,
                operator,
            });
        }
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
