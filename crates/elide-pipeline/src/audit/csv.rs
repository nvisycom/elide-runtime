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
//! - [`Table::Reviews`]: one row per reviewer override, with the
//!   rationale the provenance table has no column for.
//!
//! All three join on `entity_id`. The design choice for CSV is
//! honest scalar columns everywhere: no JSON blobs, no
//! polymorphic locations. Callers that need location details or
//! nested payloads use [`ExportJson`](elide_export::ExportJson).

use std::{io, result};

use elide::entity::Entity;
use elide::entity::audit::{Actor, Attribution, AuditKind, ManualIntent};
use elide::modality::Modality;
use elide::{Error, ErrorKind, PartId, Result};
use elide_export::{ExportCsv, Table, write_rows};
use elide_review::sorted_part_ids;
use elide_schema::each_modality;
use serde::{Serialize, Serializer};
use uuid::Uuid;

use super::Audit;

impl ExportCsv for Audit {
    /// Entities first, then their provenance, then reviewer
    /// decisions: each table joins onto the first one's
    /// `entity_id`, so this is the order a reader builds them up
    /// in.
    const TABLES: &'static [Table] = &[Table::Entities, Table::Provenance, Table::Reviews];

    /// Write one table of this audit as CSV.
    ///
    /// | table | columns |
    /// |---|---|
    /// | [`Entities`] | `part_id, modality, entity_id, label, confidence, coref` |
    /// | [`Provenance`] | `entity_id, event_index, kind, actor, actor_version, confidence, timestamp, payload_id` |
    /// | [`Reviews`] | `entity_id, intent, reviewer, authority, reason, timestamp` |
    ///
    /// All three carry `entity_id` so they join back together.
    ///
    /// [`Reviews`] is a projection of the manual events already on
    /// the report, not of the [`EditSet`] a caller applied: once
    /// applied, an override *is* part of a detection's provenance.
    /// It exists beside [`Provenance`] because that table renders
    /// every kind with one identifier column, which has nowhere to
    /// put a reviewer's rationale.
    ///
    /// Rows are sorted for stable diffs: entities by
    /// `(part_id, entity_id)`, provenance by
    /// `(entity_id, event_index)`, reviews by
    /// `(entity_id, timestamp)`.
    ///
    /// `part_id` is the part's path, segments joined for display,
    /// and `coref` is empty
    /// outside a coreference cluster.
    ///
    /// Locations and nested event payloads are dropped: CSV holds
    /// neither polymorphic locations nor event chains. Callers who
    /// need them use [`ExportJson`](elide_export::ExportJson).
    ///
    /// [`EditSet`]: elide_review::EditSet
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
    /// Iterate every entity across every part, sorted by
    /// `(part_id, entity_id)` for stable output.
    fn entity_rows(&self) -> Vec<EntityRow<'_>> {
        let mut rows = Vec::new();
        let parts = sorted_part_ids(&self.report);
        each_modality!(|M| {
            for id in &parts {
                if let Ok(entities) = self.report.part::<M>(id).map(|part| part.entities()) {
                    extend_entity_rows::<M>(entities, &encode_part_path(id), &mut rows);
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
        each_modality!(|M| {
            for id in &parts {
                if let Ok(entities) = self.report.part::<M>(id).map(|part| part.entities()) {
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

    /// One row per reviewer override, sorted by `(entity_id,
    /// timestamp)`.
    ///
    /// Empty for an audit no reviewer touched, which is the common
    /// case: an export runs whether or not a review happened, and
    /// an empty table says "nothing was overridden" rather than
    /// failing.
    fn review_rows(&self) -> Vec<ReviewRow<'_>> {
        let mut rows = Vec::new();
        let parts = sorted_part_ids(&self.report);
        each_modality!(|M| {
            for id in &parts {
                if let Ok(entities) = self.report.part::<M>(id).map(|part| part.entities()) {
                    extend_review_rows(entities, &mut rows);
                }
            }
        });
        // By timestamp within an entity, so two overrides on one
        // entity read in the order a reviewer made them.
        rows.sort_by(|a, b| {
            a.entity_id
                .cmp(&b.entity_id)
                .then_with(|| a.timestamp.cmp(&b.timestamp))
        });
        rows
    }
}

/// One entity row for CSV export. Every borrowed field points
/// into the source [`Audit`] for the lifetime of the export.
#[derive(Serialize)]
#[serde(rename_all = "snake_case")]
struct EntityRow<'a> {
    part_id: String,
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
    /// Who acted: the component's name, or the reviewer's id. Empty
    /// for a kind that records a fact rather than an act.
    actor: Option<&'a str>,
    /// The component's version when it ran. Empty for a reviewer,
    /// who has none.
    actor_version: Option<&'a str>,
    #[serde(serialize_with = "serialize_confidence")]
    confidence: f32,
    timestamp: String,
    payload_id: Option<&'a str>,
}

/// One reviewer override: which decision a human made on an
/// entity, and why.
///
/// Distinct from the [`Provenance`](Table::Provenance) row for the
/// same event, which renders the intent but drops the rationale:
/// CSV has no column for an `Attribution`, so provenance keeps the
/// one identifier every kind carries and this table unpacks the
/// one kind that carries more.
#[derive(Serialize)]
#[serde(rename_all = "snake_case")]
struct ReviewRow<'a> {
    entity_id: Uuid,
    /// `flag`, `suppress` or `amend`.
    intent: &'static str,
    /// Who made the decision, as the event names them.
    reviewer: &'a str,
    /// The cited authority, when the reviewer cited one. Empty for
    /// a freeform rationale and for an unexplained override.
    authority: Option<&'a str>,
    /// The reviewer's rationale: the citation's own rationale when
    /// cited, the policy label when freeform. Empty when the
    /// override carried no reason.
    reason: Option<&'a str>,
    timestamp: String,
}

fn extend_entity_rows<'a, M: Modality>(
    entities: &'a [Entity<M>],
    part_id: &str,
    out: &mut Vec<EntityRow<'a>>,
) {
    out.extend(entities.iter().map(|e| EntityRow {
        part_id: part_id.to_owned(),
        modality: M::NAME,
        entity_id: e.id(),
        label: e.label().as_str(),
        confidence: f32::from(e.confidence()),
        coref: e.coref().map(|c| c.as_str()),
    }));
}

fn extend_provenance_rows<'a, M: Modality>(
    entities: &'a [Entity<M>],
    out: &mut Vec<ProvenanceRow<'a>>,
) {
    for entity in entities {
        for (index, event) in entity.audit().events().iter().enumerate() {
            let columns = EventColumns::of(&event.kind);
            let (actor, actor_version) = match event.actor() {
                Some(Actor::Component(component)) => (
                    Some(component.name.as_str()),
                    Some(component.version.as_str()),
                ),
                Some(Actor::Reviewer(reviewer)) => (Some(reviewer), None),
                None => (None, None),
            };
            out.push(ProvenanceRow {
                entity_id: entity.id(),
                event_index: index,
                kind: columns.kind,
                actor,
                actor_version,
                confidence: f32::from(event.confidence),
                timestamp: event.timestamp.to_string(),
                payload_id: columns.payload_id,
            });
        }
    }
}

/// Collect one row per reviewer decision on these entities.
///
/// Walks the same provenance chains the
/// [`Provenance`](Table::Provenance) table does and keeps only a
/// reviewer's inclusion and manual events, so an entity no reviewer
/// touched contributes nothing and an entity overridden twice
/// contributes two rows.
fn extend_review_rows<'a, M: Modality>(entities: &'a [Entity<M>], out: &mut Vec<ReviewRow<'a>>) {
    for entity in entities {
        for event in entity.audit().events() {
            let (intent, reviewer, attribution) = match &event.kind {
                AuditKind::Included(included) => ("include", &included.by, &included.attribution),
                AuditKind::Manual(manual) => {
                    (intent_name(&manual.intent), &manual.by, &manual.attribution)
                }
                _ => continue,
            };
            let (authority, reason) = match attribution {
                Some(Attribution::Cited(cited)) => {
                    (Some(cited.authority.as_str()), cited.rationale.as_deref())
                }
                // A freeform attribution's `name` is where
                // `EditSet::apply` puts the reviewer's reason, so it
                // reads as the rationale rather than an authority.
                Some(Attribution::Freeform(freeform)) => (None, Some(freeform.name.as_str())),
                None => (None, None),
            };
            out.push(ReviewRow {
                entity_id: entity.id(),
                intent,
                reviewer: reviewer.as_str(),
                authority,
                reason,
                timestamp: event.timestamp.to_string(),
            });
        }
    }
}

/// The column spelling for one reviewer decision.
///
/// Shared with [`EventColumns::of`], so the `payload_id` a
/// provenance row shows for a manual event and the `intent` this
/// table shows cannot disagree.
const fn intent_name(intent: &ManualIntent) -> &'static str {
    match intent {
        ManualIntent::Suppress => "suppress",
        ManualIntent::Confirm => "confirm",
        ManualIntent::Relabel { .. } => "relabel",
        ManualIntent::Relocate => "relocate",
    }
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
            "actor",
            "actor_version",
            "confidence",
            "timestamp",
            "payload_id",
        ]
    }
}

impl ReviewRow<'_> {
    const fn header() -> &'static [&'static str] {
        &[
            "entity_id",
            "intent",
            "reviewer",
            "authority",
            "reason",
            "timestamp",
        ]
    }
}

/// How a provenance event renders in the CSV: its kind, and the
/// one identifier that kind carries.
///
/// `payload_id` is the one name the event carries beyond its actor:
/// the pattern rule, the model, the flagged or included label, a
/// manual event's intent. `None` for the kinds whose actor is the
/// whole story, and for those that carry no such name.
struct EventColumns<'a> {
    /// The event kind, as the `kind` column.
    kind: &'static str,
    /// What produced it, as the `payload_id` column.
    payload_id: Option<&'a str>,
}

impl<'a> EventColumns<'a> {
    /// A kind that names what produced it.
    const fn named(kind: &'static str, payload_id: &'a str) -> Self {
        Self {
            kind,
            payload_id: Some(payload_id),
        }
    }

    /// A kind that carries no such name.
    const fn bare(kind: &'static str) -> Self {
        Self {
            kind,
            payload_id: None,
        }
    }

    /// Read the columns off one provenance event kind.
    fn of<M: Modality>(kind: &'a AuditKind<M>) -> Self {
        match kind {
            AuditKind::Pattern(e) => Self::named("pattern", e.rule.name.as_str()),
            AuditKind::Model(e) => Self::named("model", e.model.name.as_str()),
            AuditKind::Flagged(e) => Self::named("flagged", e.label.as_str()),
            // The strategy, reconciler, layer and operator are each
            // the event's actor, which the row already carries.
            AuditKind::Fused(_) => Self::bare("fused"),
            AuditKind::Resolved(_) => Self::bare("resolved"),
            AuditKind::Contested(_) => Self::bare("contested"),
            AuditKind::Dropped(_) => Self::bare("dropped"),
            AuditKind::Calibrated(_) => Self::bare("calibrated"),
            AuditKind::Refined(_) => Self::bare("refined"),
            AuditKind::Redacted(_) => Self::bare("redacted"),
            AuditKind::Selected(_) => Self::bare("selected"),
            AuditKind::Included(e) => Self::named("included", e.label.as_str()),
            // The reviewer is the row's `actor`; what distinguishes one
            // manual event from another is which decision it recorded.
            AuditKind::Manual(e) => Self::named("manual", intent_name(&e.intent)),
            AuditKind::Annotated(_) => Self::bare("annotated"),
            // `AuditKind` is `#[non_exhaustive]`: a kind added upstream
            // lands here rather than breaking the build. Every variant
            // elide ships today is named above, so this arm firing means
            // a new one needs a column mapping.
            _ => Self::bare("unknown"),
        }
    }
}

/// Format a `f32` confidence with three decimal places.
fn serialize_confidence<S: Serializer>(value: &f32, s: S) -> result::Result<S::Ok, S::Error> {
    s.serialize_str(&format!("{value:.3}"))
}

/// One part path as a CSV cell: its segments as a JSON array.
///
/// Not [`PartId`]'s `Display`, which joins on `\u{203a}` and is
/// documented upstream as "a display convenience for error messages
/// and logs, never parsed back". A CSV is machine-read, and a
/// segment may itself contain any delimiter, so the cell carries the
/// same segment array the wire form uses and a consumer parses it
/// back rather than splitting on a separator that may change.
fn encode_part_path(id: &PartId) -> String {
    let segments: Vec<&str> = id.segments().collect();
    serde_json::to_string(&segments).unwrap_or_else(|_| id.to_string())
}
