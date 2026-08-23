//! Modality-tagged group of recognised entities.
//!
//! [`EntityGroup`] is the unit [`Audit`] stores in `body` and in
//! every `parts` entry: one enum tagged by `modality` (snake_case)
//! so deserialisation picks the right variant and the entity vec
//! inside is statically typed per modality. Apply-time we hand
//! each variant back to elide as a `Vec<Entity<M>>` for the
//! appropriate `M`.
//!
//! ## Plumbing between [`EntityGroup`] and elide's [`Report`]
//!
//! Three directions:
//!
//! - **Drain**: after [`Orchestrator::analyze`] returns a
//!   [`Report`], [`take_body`] and [`take_part`] move each typed
//!   `Vec<Entity<M>>` out of the report into the matching
//!   [`EntityGroup`] variant.
//! - **Rebuild**: at anonymize time,
//!   [`EntityGroup::insert_into_body`] and
//!   [`EntityGroup::insert_as_part`] feed a fresh [`Report`] from
//!   the returned groups (cloning entities; the returned body is
//!   the source of truth for re-apply idempotency).
//! - **Merge**: after [`Orchestrator::anonymize_with`] returns
//!   its mutated [`Report`], [`EntityGroup::merge_body_from`] and
//!   [`EntityGroup::merge_part_from`] move the redaction events
//!   elide stamped onto each entity's provenance chain back onto
//!   the caller's records.
//!
//! Plus two byte-level helpers used at the anonymize seam:
//! [`EntityGroup::collect_overrides_into`] walks reviewer
//! overrides off any group, and
//! [`EntityGroup::encode_redacted_from`] picks the right typed
//! handle to re-encode through after `anonymize_with` mutated the
//! document in place.
//!
//! Per-modality construction lives on one trait
//! ([`GroupCarrier`]) with a macro-generated impl per modality,
//! reused by [`take_body`] / [`take_part`] to fold four
//! identical bodies into one generic function.
//! Every [`EntityGroup`] method dispatches on variant via the
//! internal `dispatch!` macro, so the modality list lives in
//! exactly one place: the macro definition. Adding a fifth
//! modality means updating the enum, `impl_group_carrier!`, and
//! one macro arm.
//!
//! [`Audit`]: crate::Audit
//! [`Orchestrator::analyze`]: elide::Orchestrator::analyze
//! [`Orchestrator::anonymize_with`]: elide::Orchestrator::anonymize_with
//! [`Report`]: elide::Report

use std::any::Any;
use std::collections::HashMap;
use std::mem;

use bytes::Bytes;
use elide::codec::{PartId, UntypedDocumentHandle};
use elide::entity::Entity;
use elide::entity::audit::{AuditEvent, AuditKind};
use elide::modality::Modality;
use elide::modality::audio::Audio;
use elide::modality::image::Image;
use elide::modality::tabular::Tabular;
use elide::modality::text::Text;
use elide::recognition::Scope;
use elide::redaction::Anonymizer;
use elide::{Error, ErrorKind, Report, Result};
use elide_governance::modality::RedactableModality;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::overrides::OverrideSet;
use super::record::{EntityRecord, OverrideEntry, Review};
use crate::pipeline::Picker;

/// A modality-tagged group of recognised entities.
///
/// The unit [`Audit`] stores in `body` and in every `parts`
/// entry.
///
/// Tagged by `modality` (snake_case) so deserialization picks the
/// right variant and the entity vec inside is statically typed
/// per modality: apply-time we hand each variant back to elide
/// as a `Vec<Entity<M>>` for the appropriate `M`.
///
/// [`Audit`]: crate::Audit
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "modality", content = "entities", rename_all = "snake_case")]
pub enum EntityGroup {
    /// Text entities, in source-coordinate order.
    Text(Vec<EntityRecord<Text>>),
    /// Tabular entities, in source-coordinate order.
    Tabular(Vec<EntityRecord<Tabular>>),
    /// Image entities, in source-coordinate order.
    Image(Vec<EntityRecord<Image>>),
    /// Audio entities, in source-coordinate order.
    Audio(Vec<EntityRecord<Audio>>),
}

/// Per-modality bridge from a drained `Vec<Entity<M>>` back
/// into an [`EntityGroup`] variant. Implemented once per modality
/// via macro; consumed by the drain helpers that walk modalities
/// in fallthrough order.
pub(crate) trait GroupCarrier: Modality + Sized + 'static {
    /// Wrap a drained `Vec<Entity<Self>>` into the matching
    /// [`EntityGroup`] variant.
    fn into_group(entities: Vec<Entity<Self>>) -> EntityGroup;
}

macro_rules! impl_group_carrier {
    ($modality:ty, $variant:ident) => {
        impl GroupCarrier for $modality {
            fn into_group(entities: Vec<Entity<Self>>) -> EntityGroup {
                EntityGroup::$variant(entities.into_iter().map(EntityRecord::new).collect())
            }
        }
    };
}

impl_group_carrier!(Text, Text);
impl_group_carrier!(Tabular, Tabular);
impl_group_carrier!(Image, Image);
impl_group_carrier!(Audio, Audio);

/// Drain the body's entities from `report` into an
/// [`EntityGroup`] of `M`'s variant, or `None` if the body is a
/// different modality.
pub(crate) fn take_body<M: GroupCarrier>(report: &mut Report) -> Option<EntityGroup> {
    let entities = mem::take(report.entities::<M>()?);
    Some(M::into_group(entities))
}

/// Drain the part `id`'s entities into an [`EntityGroup`] of
/// `M`'s variant, or `None` if `M` is not that part's modality.
pub(crate) fn take_part<M: GroupCarrier>(report: &mut Report, id: &PartId) -> Option<EntityGroup> {
    let entities = mem::take(report.part_entities::<M>(id)?);
    Some(M::into_group(entities))
}

/// Dispatch on an [`EntityGroup`] variant, binding the modality
/// type as `$m` and the entities slot as `$entities` in the body.
/// The modality list lives here: every method on [`EntityGroup`]
/// below reuses this macro instead of re-writing four match
/// arms.
///
/// Every method that consumes this macro uses both bindings;
/// pattern-bind the entities slot to `_` in an arm if a caller
/// ever needs only the modality type.
macro_rules! dispatch {
    ($group:expr, |$m:ident, $entities:tt| $body:expr) => {{
        match $group {
            EntityGroup::Text($entities) => {
                type $m = Text;
                $body
            }
            EntityGroup::Tabular($entities) => {
                type $m = Tabular;
                $body
            }
            EntityGroup::Image($entities) => {
                type $m = Image;
                $body
            }
            EntityGroup::Audio($entities) => {
                type $m = Audio;
                $body
            }
        }
    }};
}

impl EntityGroup {
    /// Add an entity a reviewer spotted that recognition missed.
    ///
    /// Its human origin is made auditable: unless `entity` already
    /// carries one, a [`Manual`] event built from its own location
    /// and confidence joins its trail as it is added, so an
    /// included entity is never mistaken for an automatic
    /// detection. It is then redacted like any detected one.
    ///
    /// Returns `false`, adding nothing, when `M` is not this
    /// group's modality: an image entity cannot join a text group.
    ///
    /// [`Manual`]: elide::entity::audit::AuditKind::Manual
    pub fn include<M: RedactableModality>(&mut self, entity: Entity<M>) -> bool {
        let Some(entities) = self.entities_mut::<M>() else {
            return false;
        };
        let mut record = EntityRecord::new(entity);
        if !record.entity.audit.events().iter().any(is_manual) {
            let event = AuditEvent::manual_include(
                record.entity.location.clone(),
                record.entity.confidence,
            );
            record.entity.audit.record(event);
        }
        entities.push(record);
        true
    }

    /// This group's records as `EntityRecord<M>`, or `None` when
    /// `M` is not this group's modality.
    ///
    /// The `dyn Any` downcast that lets a modality-generic caller
    /// reach into the tagged enum. Kept private: callers go
    /// through [`include`](Self::include).
    fn entities_mut<M: RedactableModality>(&mut self) -> Option<&mut Vec<EntityRecord<M>>> {
        dispatch!(self, |_M, entities| (entities as &mut dyn Any)
            .downcast_mut::<Vec<EntityRecord<M>>>())
    }

    /// Record each entity's operator pick onto its own audit
    /// trail, using the anonymizer for this group's modality.
    ///
    /// A suppressed entity is skipped by elide's own clustering,
    /// so it gains no pick: nothing was going to redact it.
    pub(crate) fn record_picks(&mut self, picker: &Picker, scope: &Scope) {
        match self {
            Self::Text(records) => pick_into(&picker.text, records, scope),
            Self::Tabular(records) => pick_into(&picker.tabular, records, scope),
            Self::Image(records) => pick_into(&picker.image, records, scope),
            Self::Audio(records) => pick_into(&picker.audio, records, scope),
        }
    }

    /// Insert this group into `report` as the body under its
    /// modality.
    pub(crate) fn insert_into_body(&self, report: Report) -> Report {
        dispatch!(self, |M, entities| report
            .insert_body::<M>(clone_entities(entities)))
    }

    /// Insert this group into `report` as a container part keyed
    /// by `id`.
    pub(crate) fn insert_as_part(&self, report: Report, id: &str) -> Report {
        let part_id = PartId::from(id.to_owned());
        dispatch!(self, |M, entities| {
            report.insert_part::<M>(part_id, clone_entities(entities))
        })
    }

    /// Append every reviewer override on this group to `out` as
    /// [`OverrideEntry`] triples (entity id + authoring policy
    /// id + operator spec). Records without a review pane are
    /// skipped.
    ///
    /// [`OverrideEntry`]: super::record::OverrideEntry
    pub(crate) fn collect_overrides_into(&self, out: &mut OverrideSet) {
        match self {
            Self::Text(entities) => extend_overrides(&mut out.text, entities),
            Self::Tabular(entities) => extend_overrides(&mut out.tabular, entities),
            Self::Image(entities) => extend_overrides(&mut out.image, entities),
            Self::Audio(entities) => extend_overrides(&mut out.audio, entities),
        }
    }

    /// Stamp every pending [`Review::Suppress`] onto its entity's
    /// audit trail, so elide's redaction pass skips it.
    ///
    /// Called once before the report is rebuilt. Idempotent: an
    /// entity elide already sees as suppressed is left alone, so
    /// re-applying an audit does not stack duplicate events on the
    /// trail.
    pub(crate) fn apply_suppressions(&mut self) {
        match self {
            Self::Text(records) => stamp_suppressions(records),
            Self::Tabular(records) => stamp_suppressions(records),
            Self::Image(records) => stamp_suppressions(records),
            Self::Audio(records) => stamp_suppressions(records),
        }
    }

    /// Merge post-apply provenance from `report`'s body into this
    /// group's records, matched by entity id.
    ///
    /// After [`Orchestrator::anonymize_with`] applies operators,
    /// each mutated `Entity<M>` on the returned report carries a
    /// fresh redaction event appended to its
    /// `provenance.events`. This helper moves that chain onto the
    /// caller's [`EntityRecord<M>`] so the audit is visible via
    /// `audit.body`.
    ///
    /// [`Orchestrator::anonymize_with`]: elide::Orchestrator::anonymize_with
    pub(crate) fn merge_body_from(&mut self, report: &mut Report) {
        dispatch!(self, |M, entities| {
            merge_provenance::<M>(entities, |f| report.for_each_body_mut::<M>(f));
        })
    }

    /// Merge post-apply provenance for a container part. Same
    /// shape as [`Self::merge_body_from`], keyed by part id.
    pub(crate) fn merge_part_from(&mut self, report: &mut Report, id: &str) {
        let part_id = PartId::from(id.to_owned());
        dispatch!(self, |M, entities| {
            merge_provenance::<M>(entities, |f| report.for_each_part_mut::<M>(&part_id, f));
        })
    }

    /// Re-encode `handle` into raw bytes using this group's
    /// modality as the typed encode target.
    ///
    /// Called after `anonymize_with` mutated `handle` in place.
    /// The apply-time re-encode needs the typed form because
    /// [`elide::codec::DocumentHandle::encode`] is per-modality.
    pub(crate) fn encode_redacted_from(&self, handle: UntypedDocumentHandle) -> Result<Bytes> {
        dispatch!(self, |M, _entities| encode_typed::<M>(handle))
    }
}

/// Whether an event records a human override, in either
/// direction. Used to avoid stamping a second [`Manual`] event
/// onto an entity a caller already marked.
///
/// [`Manual`]: elide::entity::audit::AuditKind::Manual
fn is_manual<M: Modality>(event: &AuditEvent<M>) -> bool {
    matches!(event.kind, AuditKind::Manual(_))
}

/// Run `anonymizer`'s pick pass over `records`' entities.
///
/// [`Anonymizer::pick`] wants a contiguous `&mut [Entity<M>]`,
/// but these entities are fields inside [`EntityRecord`]s. Taking
/// the whole vec, picking over it, and putting it back is the one
/// move that avoids needing a dummy `Entity` to swap in per
/// record. `pick` only appends audit events, never reorders or
/// resizes, so the zip back stays aligned.
///
/// [`Anonymizer::pick`]: elide::redaction::Anonymizer::pick
fn pick_into<M: RedactableModality + 'static>(
    anonymizer: &Anonymizer<M>,
    records: &mut Vec<EntityRecord<M>>,
    scope: &Scope,
) {
    let taken = mem::take(records);
    let (mut entities, reviews): (Vec<_>, Vec<_>) =
        taken.into_iter().map(|r| (r.entity, r.review)).unzip();
    anonymizer.pick(&mut entities, scope);
    *records = entities
        .into_iter()
        .zip(reviews)
        .map(|(entity, review)| EntityRecord { entity, review })
        .collect();
}

fn clone_entities<M: RedactableModality>(records: &[EntityRecord<M>]) -> Vec<Entity<M>>
where
    Entity<M>: Clone,
{
    records.iter().map(|r| r.entity.clone()).collect()
}

/// Stamp each record whose review is a [`Review::Suppress`] with
/// the [`Manual`] event elide reads to skip it, carrying the
/// reviewer's reason and actor.
///
/// [`Manual`]: elide::entity::audit::AuditKind::Manual
fn stamp_suppressions<M: RedactableModality>(records: &mut [EntityRecord<M>]) {
    for record in records {
        let wants_suppression = matches!(record.review, Some(Review::Suppress { .. }));
        if wants_suppression == record.entity.is_suppressed() {
            // Already in the wanted state: stamping again would
            // stack a duplicate event every time an audit is
            // re-applied.
            continue;
        }
        let location = record.entity.location.clone();
        let confidence = record.entity.confidence;
        match &record.review {
            Some(Review::Suppress { reason, actor }) => {
                let mut event = AuditEvent::manual_suppress(location, confidence);
                if let Some(reason) = reason {
                    event = event.with_reason(reason.clone());
                }
                if let Some(actor) = actor {
                    event = event.with_actor(actor.clone());
                }
                record.entity.suppress(event);
            }
            // The reviewer took a suppression back, by deciding to
            // redact after all or by clearing the decision. Recording
            // the reversal rather than rewriting history: `is_suppressed`
            // reads the most recent Manual event, so an include event
            // lifts the earlier suppress and the trail keeps both halves
            // of the reviewer's change of mind.
            Some(Review::Redact { .. }) | None => {
                record
                    .entity
                    .audit
                    .record(AuditEvent::manual_include(location, confidence));
            }
        }
    }
}

fn extend_overrides<M: RedactableModality>(
    out: &mut Vec<OverrideEntry<M>>,
    records: &[EntityRecord<M>],
) {
    // Only `Redact` yields an override entry: a `Suppress` names no
    // operator, and is applied by stamping the entity itself (see
    // `apply_suppressions`).
    out.extend(records.iter().filter_map(|r| match &r.review {
        Some(Review::Redact { policy_id, action }) => Some(OverrideEntry {
            entity_id: r.entity.id,
            policy_id: *policy_id,
            action: action.clone(),
        }),
        Some(Review::Suppress { .. }) | None => None,
    }));
}

/// Index `records` by id, then let `walk_mutated` invoke the
/// per-entity callback once per mutated `Entity<M>`. For each
/// mutated entity whose id matches a record, `mem::take` moves
/// its provenance chain onto the record. O(n + m) with one
/// HashMap allocation, no per-mutated linear scan.
fn merge_provenance<M: RedactableModality>(
    records: &mut [EntityRecord<M>],
    walk_mutated: impl FnOnce(&mut dyn FnMut(&mut Entity<M>)),
) {
    let mut by_id: HashMap<Uuid, &mut EntityRecord<M>> =
        records.iter_mut().map(|r| (r.entity.id, r)).collect();
    walk_mutated(&mut |entity| {
        if let Some(record) = by_id.get_mut(&entity.id) {
            record.entity.audit = mem::take(&mut entity.audit);
        }
    });
}

fn encode_typed<M: Modality>(handle: UntypedDocumentHandle) -> Result<Bytes> {
    let typed = handle.into::<M>().map_err(|_| {
        Error::new(
            ErrorKind::Redaction,
            "post-apply re-encode: handle modality does not match the audit's body group",
        )
    })?;
    let content = typed.encode().map_err(|err| {
        Error::new(
            ErrorKind::Redaction,
            format!("post-apply encode failed: {err}"),
        )
    })?;
    Ok(content.into_bytes())
}
