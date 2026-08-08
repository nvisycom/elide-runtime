//! CSV export for [`Audit`]. Gated on the `audit-csv` cargo
//! feature.
//!
//! Three files, three writers, one join key. Each writes to any
//! `impl Write`; callers decide where the bytes land (a file,
//! an HTTP body, an S3 upload):
//!
//! - [`Audit::write_entities_csv`] — one row per detected
//!   entity. Scalar essentials only.
//! - [`Audit::write_provenance_csv`] — one row per event on
//!   every entity's provenance chain.
//! - [`Audit::write_reviews_csv`] — one row per reviewed entity.
//!
//! All three join on `entity_id`. The design choice for CSV is
//! honest scalar columns everywhere — no JSON blobs, no
//! polymorphic locations. Callers that need location details or
//! nested payloads use [`Audit::write_json`].

use std::{io, result};

use elide_core::entity::provenance::EventKind;
use elide_core::modality::Modality;
use elide_core::{Error, ErrorKind, Result};
use nvisy_schema::policy::redaction::ModalityRedactions;
use serde::{Serialize, Serializer};
use uuid::Uuid;

use super::audit::Audit;
use crate::entity::{EntityGroup, EntityRecord};

impl Audit {
    /// Serialize the entity table as CSV into `writer`.
    ///
    /// Columns: `part_id, modality, entity_id, label,
    /// confidence, coref`. One row per detected entity, sorted
    /// by `(part_id, entity_id)` for stable diffs. `part_id` is
    /// empty for body entities. `coref` is empty when the
    /// entity isn't part of a coreference cluster.
    ///
    /// Joins with [`Self::write_provenance_csv`] and
    /// [`Self::write_reviews_csv`] on `entity_id`.
    ///
    /// Location details and provenance are dropped from this
    /// export — CSV can't cleanly hold polymorphic locations or
    /// nested event chains. Callers that need those use
    /// [`Self::write_json`].
    ///
    /// # Errors
    ///
    /// Returns [`ErrorKind::Processing`] wrapping the underlying
    /// [`csv::Error`] on I/O or serialization failure.
    pub fn write_entities_csv<W: io::Write>(&self, writer: W) -> Result<()> {
        #[derive(Serialize)]
        #[serde(rename_all = "snake_case")]
        struct Row<'a> {
            part_id: Option<&'a str>,
            modality: &'a str,
            entity_id: Uuid,
            label: &'a str,
            #[serde(serialize_with = "serialize_confidence")]
            confidence: f32,
            coref: Option<&'a str>,
        }

        let mut csv = csv::WriterBuilder::new()
            .has_headers(false)
            .from_writer(writer);
        csv.write_record([
            "part_id",
            "modality",
            "entity_id",
            "label",
            "confidence",
            "coref",
        ])
        .map_err(csv_err)?;
        for row in self.entity_rows() {
            csv.serialize(Row {
                part_id: row.part_id,
                modality: row.modality,
                entity_id: row.entity_id,
                label: row.label,
                confidence: row.confidence,
                coref: row.coref,
            })
            .map_err(csv_err)?;
        }
        csv.flush().map_err(io_err)
    }

    /// Serialize the provenance table as CSV into `writer`.
    ///
    /// Columns: `entity_id, event_index, kind, source, before,
    /// after, at, payload_id`. One row per event, sorted by
    /// `entity_id` then `event_index`. `before` is empty on the
    /// first (birth) event; `payload_id` carries the model name
    /// or pattern rule id when the event's `kind` has one,
    /// empty otherwise.
    ///
    /// Joins with [`Self::write_entities_csv`] on `entity_id`.
    ///
    /// # Errors
    ///
    /// Returns [`ErrorKind::Processing`] on I/O or serialization
    /// failure.
    pub fn write_provenance_csv<W: io::Write>(&self, writer: W) -> Result<()> {
        #[derive(Serialize)]
        #[serde(rename_all = "snake_case")]
        struct Row<'a> {
            entity_id: Uuid,
            event_index: usize,
            kind: &'a str,
            source: &'a str,
            #[serde(serialize_with = "serialize_optional_confidence")]
            before: Option<f32>,
            #[serde(serialize_with = "serialize_confidence")]
            after: f32,
            at: String,
            payload_id: Option<&'a str>,
        }

        let mut csv = csv::WriterBuilder::new()
            .has_headers(false)
            .from_writer(writer);
        csv.write_record([
            "entity_id",
            "event_index",
            "kind",
            "source",
            "before",
            "after",
            "at",
            "payload_id",
        ])
        .map_err(csv_err)?;
        for row in self.provenance_rows() {
            csv.serialize(Row {
                entity_id: row.entity_id,
                event_index: row.event_index,
                kind: row.kind,
                source: row.source,
                before: row.before,
                after: row.after,
                at: row.at,
                payload_id: row.payload_id,
            })
            .map_err(csv_err)?;
        }
        csv.flush().map_err(io_err)
    }

    /// Serialize the review table as CSV into `writer`.
    ///
    /// Columns: `entity_id, modality, operator`. One row per
    /// reviewed entity — entities without a reviewer override
    /// are absent from the file. `operator` is the kind
    /// discriminator name (`erase`, `mask`, `fake`, ...);
    /// operator parameters are dropped.
    ///
    /// Joins with [`Self::write_entities_csv`] on `entity_id`.
    ///
    /// # Errors
    ///
    /// Returns [`ErrorKind::Processing`] on I/O or serialization
    /// failure.
    pub fn write_reviews_csv<W: io::Write>(&self, writer: W) -> Result<()> {
        #[derive(Serialize)]
        #[serde(rename_all = "snake_case")]
        struct Row {
            entity_id: Uuid,
            modality: &'static str,
            operator: String,
        }

        let mut csv = csv::WriterBuilder::new()
            .has_headers(false)
            .from_writer(writer);
        csv.write_record(["entity_id", "modality", "operator"])
            .map_err(csv_err)?;
        for (entity_id, modality, operator) in self.review_rows() {
            csv.serialize(Row {
                entity_id,
                modality,
                operator,
            })
            .map_err(csv_err)?;
        }
        csv.flush().map_err(io_err)
    }

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

    /// Iterate every reviewed entity across body + parts,
    /// yielding `(entity_id, modality, operator_kind)`.
    fn review_rows(&self) -> Vec<(Uuid, &'static str, String)> {
        let mut rows: Vec<(Uuid, &'static str, String)> = Vec::new();
        if let Some(group) = &self.body {
            push_review_rows(group, &mut rows);
        }
        let mut part_keys: Vec<&String> = self.parts.keys().collect();
        part_keys.sort();
        for id in part_keys {
            push_review_rows(&self.parts[id], &mut rows);
        }
        rows.sort_by_key(|(id, _, _)| *id);
        rows
    }
}

/// One entity row for CSV export. Every borrowed field points
/// into the source [`Audit`] for the lifetime of the export.
struct EntityRow<'a> {
    part_id: Option<&'a str>,
    modality: &'a str,
    entity_id: Uuid,
    label: &'a str,
    confidence: f32,
    coref: Option<&'a str>,
}

/// One provenance row.
struct ProvenanceRow<'a> {
    entity_id: Uuid,
    event_index: usize,
    kind: &'static str,
    source: &'a str,
    before: Option<f32>,
    after: f32,
    at: String,
    payload_id: Option<&'a str>,
}

fn push_entity_rows<'a>(
    group: &'a EntityGroup,
    part_id: Option<&'a str>,
    out: &mut Vec<EntityRow<'a>>,
) {
    match group {
        EntityGroup::Text(entities) => extend_entity_rows(entities, part_id, "text", out),
        #[cfg(feature = "internal_tabular")]
        EntityGroup::Tabular(entities) => extend_entity_rows(entities, part_id, "tabular", out),
        #[cfg(feature = "internal_image")]
        EntityGroup::Image(entities) => extend_entity_rows(entities, part_id, "image", out),
        #[cfg(feature = "internal_audio")]
        EntityGroup::Audio(entities) => extend_entity_rows(entities, part_id, "audio", out),
    }
}

fn push_provenance_rows<'a>(group: &'a EntityGroup, out: &mut Vec<ProvenanceRow<'a>>) {
    match group {
        EntityGroup::Text(entities) => extend_provenance_rows(entities, out),
        #[cfg(feature = "internal_tabular")]
        EntityGroup::Tabular(entities) => extend_provenance_rows(entities, out),
        #[cfg(feature = "internal_image")]
        EntityGroup::Image(entities) => extend_provenance_rows(entities, out),
        #[cfg(feature = "internal_audio")]
        EntityGroup::Audio(entities) => extend_provenance_rows(entities, out),
    }
}

fn push_review_rows(group: &EntityGroup, out: &mut Vec<(Uuid, &'static str, String)>) {
    match group {
        EntityGroup::Text(entities) => extend_review_rows(entities, "text", out),
        #[cfg(feature = "internal_tabular")]
        EntityGroup::Tabular(entities) => extend_review_rows(entities, "tabular", out),
        #[cfg(feature = "internal_image")]
        EntityGroup::Image(entities) => extend_review_rows(entities, "image", out),
        #[cfg(feature = "internal_audio")]
        EntityGroup::Audio(entities) => extend_review_rows(entities, "audio", out),
    }
}

fn extend_entity_rows<'a, M: Modality>(
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

fn extend_provenance_rows<'a, M: Modality>(
    records: &'a [EntityRecord<M>],
    out: &mut Vec<ProvenanceRow<'a>>,
) {
    for r in records {
        for (i, event) in r.entity.provenance.events.iter().enumerate() {
            let (kind, payload_id) = event_kind_and_payload(&event.kind);
            out.push(ProvenanceRow {
                entity_id: r.entity.id,
                event_index: i,
                kind,
                source: event.source.as_str(),
                before: event.before.map(f32::from),
                after: f32::from(event.after),
                at: event.at.to_string(),
                payload_id,
            });
        }
    }
}

fn extend_review_rows<M: Modality>(
    records: &[EntityRecord<M>],
    modality: &'static str,
    out: &mut Vec<(Uuid, &'static str, String)>,
) {
    for r in records {
        if let Some(review) = &r.review
            && let Some(op) = operator_kind_for_modality(review, modality)
        {
            out.push((r.entity.id, modality, op));
        }
    }
}

/// Discriminator + optional payload id for a provenance event
/// kind. Payload id carries the pattern rule name or model name
/// when the variant has one; empty otherwise.
fn event_kind_and_payload<M: Modality>(kind: &EventKind<M>) -> (&'static str, Option<&str>) {
    match kind {
        EventKind::Pattern { pattern, .. } => ("pattern", Some(pattern.name.as_str())),
        EventKind::Model { model, .. } => ("model", Some(model.name.as_str())),
        EventKind::Deduplication { strategy } => ("deduplication", Some(strategy.as_str())),
        EventKind::Conflict { resolved_by, .. } => ("conflict", Some(resolved_by.as_str())),
        EventKind::Contested { flagged_by, .. } => ("contested", Some(flagged_by.as_str())),
        EventKind::Calibration { .. } => ("calibration", None),
        EventKind::Refinement { .. } => ("refinement", None),
        EventKind::Redaction { .. } => ("redaction", None),
        _ => ("unknown", None),
    }
}

/// Extract the operator `kind` discriminator from the modality
/// slot on a review's [`ModalityRedactions`]. Uses serde JSON
/// as a universal `kind` reader across the four operator enums
/// — each one is `#[serde(tag = "kind")]` so the top-level JSON
/// object always has a `"kind"` field.
fn operator_kind_for_modality(review: &ModalityRedactions, modality: &str) -> Option<String> {
    let value = match modality {
        "text" => review
            .text
            .as_ref()
            .and_then(|op| serde_json::to_value(op).ok()),
        "tabular" => review
            .tabular
            .as_ref()
            .and_then(|op| serde_json::to_value(op).ok()),
        "image" => review
            .image
            .as_ref()
            .and_then(|op| serde_json::to_value(op).ok()),
        "audio" => review
            .audio
            .as_ref()
            .and_then(|op| serde_json::to_value(op).ok()),
        _ => None,
    }?;
    value
        .get("kind")
        .and_then(|k| k.as_str())
        .map(|s| s.to_owned())
}

/// Format a `f32` confidence with three decimal places.
fn serialize_confidence<S: Serializer>(value: &f32, s: S) -> result::Result<S::Ok, S::Error> {
    s.serialize_str(&format!("{value:.3}"))
}

/// Format an `Option<f32>` confidence; `None` serializes to an
/// empty string (CSV column left blank).
fn serialize_optional_confidence<S: Serializer>(
    value: &Option<f32>,
    s: S,
) -> result::Result<S::Ok, S::Error> {
    match value {
        Some(v) => s.serialize_str(&format!("{v:.3}")),
        None => s.serialize_str(""),
    }
}

/// Map a `csv::Error` into an engine [`Error`].
fn csv_err(err: csv::Error) -> Error {
    Error::new(
        ErrorKind::Processing,
        format!("audit CSV export failed: {err}"),
    )
}

/// Map a `std::io::Error` into an engine [`Error`].
fn io_err(err: io::Error) -> Error {
    Error::new(
        ErrorKind::Processing,
        format!("audit CSV export flush failed: {err}"),
    )
}
