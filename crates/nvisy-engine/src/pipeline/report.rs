//! Plumbing between [`Audit`] / [`RecognizedGroup`] and
//! elide's runtime [`Report`].
//!
//! Three directions:
//!
//! - **Drain**: after [`Orchestrator::analyze`] returns a
//!   [`Report`], [`take_body`] and [`take_part`] move each typed
//!   `Vec<Entity<M>>` out of the report into the matching
//!   [`RecognizedGroup`] variant for the caller to hold.
//! - **Rebuild**: at anonymize time,
//!   [`RecognizedGroup::insert_into_body`] and
//!   [`RecognizedGroup::insert_as_part`] feed a fresh [`Report`]
//!   from the returned groups (cloning entities; the returned
//!   body is the source of truth for re-apply idempotency).
//! - **Merge**: after [`Orchestrator::anonymize_with`] returns
//!   its mutated [`Report`],
//!   [`RecognizedGroup::merge_body_from`] and
//!   [`RecognizedGroup::merge_part_from`] move the redaction
//!   events elide stamped onto each entity's provenance chain
//!   back onto the caller's records.
//!
//! Plus two byte-level helpers used at the anonymize seam:
//! [`RecognizedGroup::collect_overrides_into`] walks reviewer
//! overrides off any group, and
//! [`RecognizedGroup::encode_redacted_from`] picks the right
//! typed handle to re-encode through after `anonymize_with`
//! mutated the document in place.
//!
//! Per-modality construction lives on one trait
//! ([`GroupCarrier`]) with a macro-generated impl per modality,
//! reused by [`take_body`] / [`take_part`] to fold four
//! feature-gated identical bodies into one generic function.
//! Every [`RecognizedGroup`] method dispatches on variant via
//! the internal [`dispatch!`] macro, so the modality list lives
//! in exactly one place: the macro definition. Adding a fifth
//! modality means updating the enum, [`impl_group_carrier!`],
//! and one macro arm.
//!
//! [`Audit`]: crate::Audit
//! [`RecognizedGroup`]: crate::RecognizedGroup
//! [`Engine`]: super::Engine
//! [`Orchestrator::analyze`]: elide::Orchestrator::analyze
//! [`Orchestrator::anonymize_with`]: elide::Orchestrator::anonymize_with
//! [`Report`]: elide::Report

use std::collections::HashMap;
use std::mem;

use bytes::Bytes;
use elide::Report;
use elide::codec::{PartId, UntypedDocumentHandle};
use elide_core::entity::Entity;
use elide_core::modality::Modality;
#[cfg(feature = "internal_audio")]
use elide_core::modality::audio::Audio;
#[cfg(feature = "internal_image")]
use elide_core::modality::image::Image;
#[cfg(feature = "internal_tabular")]
use elide_core::modality::tabular::Tabular;
use elide_core::modality::text::Text;
use elide_core::{Error, ErrorKind, Result};
use nvisy_schema::policy::redaction::ModalityRedactions;
use uuid::Uuid;

use super::audit::{EntityRecord, RecognizedGroup};

/// Per-modality bridge from a drained `Vec<Entity<M>>` back
/// into a [`RecognizedGroup`] variant. Implemented once per
/// modality via macro; consumed by the drain helpers that walk
/// modalities in feature-gated fallthrough order.
pub(super) trait GroupCarrier: Modality + Sized + 'static {
    /// Wrap a drained `Vec<Entity<Self>>` into the matching
    /// [`RecognizedGroup`] variant.
    fn into_group(entities: Vec<Entity<Self>>) -> RecognizedGroup;
}

macro_rules! impl_group_carrier {
    ($modality:ty, $variant:ident) => {
        impl GroupCarrier for $modality {
            fn into_group(entities: Vec<Entity<Self>>) -> RecognizedGroup {
                RecognizedGroup::$variant {
                    entities: entities.into_iter().map(EntityRecord::new).collect(),
                }
            }
        }
    };
}

impl_group_carrier!(Text, Text);
#[cfg(feature = "internal_tabular")]
impl_group_carrier!(Tabular, Tabular);
#[cfg(feature = "internal_image")]
impl_group_carrier!(Image, Image);
#[cfg(feature = "internal_audio")]
impl_group_carrier!(Audio, Audio);

/// Drain the body's entities from `report` into a
/// [`RecognizedGroup`] of `M`'s variant, or `None` if the body
/// is a different modality.
pub(super) fn take_body<M: GroupCarrier>(report: &mut Report) -> Option<RecognizedGroup> {
    let entities = mem::take(report.entities::<M>()?);
    Some(M::into_group(entities))
}

/// Drain the part `id`'s entities into a [`RecognizedGroup`] of
/// `M`'s variant, or `None` if `M` is not that part's modality.
pub(super) fn take_part<M: GroupCarrier>(
    report: &mut Report,
    id: &PartId,
) -> Option<RecognizedGroup> {
    let entities = mem::take(report.part_entities::<M>(id)?);
    Some(M::into_group(entities))
}

/// Dispatch on a [`RecognizedGroup`] variant, binding the
/// modality type as `$m` and the entities slot as `$entities`
/// in the body. The modality list lives here — every method on
/// [`RecognizedGroup`] below reuses this macro instead of
/// re-writing four feature-gated match arms.
///
/// Every method that consumes this macro uses both bindings;
/// pattern-bind the entities slot to `_` in an arm if a caller
/// ever needs only the modality type.
macro_rules! dispatch {
    ($group:expr, |$m:ident, $entities:tt| $body:expr) => {{
        // Bring the modality types into scope with unique aliases
        // so referencing `$m` in the body picks up the arm's
        // concrete type via redefinition below. Rust never
        // complains about an unused type alias.
        match $group {
            RecognizedGroup::Text { entities: $entities } => {
                type $m = Text;
                $body
            }
            #[cfg(feature = "internal_tabular")]
            RecognizedGroup::Tabular { entities: $entities } => {
                type $m = Tabular;
                $body
            }
            #[cfg(feature = "internal_image")]
            RecognizedGroup::Image { entities: $entities } => {
                type $m = Image;
                $body
            }
            #[cfg(feature = "internal_audio")]
            RecognizedGroup::Audio { entities: $entities } => {
                type $m = Audio;
                $body
            }
        }
    }};
}

impl RecognizedGroup {
    /// Insert this group into `report` as the body under its
    /// modality.
    pub(super) fn insert_into_body(&self, report: Report) -> Report {
        dispatch!(self, |M, entities| report.insert_body::<M>(clone_entities(entities)))
    }

    /// Insert this group into `report` as a container part
    /// keyed by `id`.
    pub(super) fn insert_as_part(&self, report: Report, id: &str) -> Report {
        let part_id = PartId::from(id.to_owned());
        dispatch!(self, |M, entities| {
            report.insert_part::<M>(part_id, clone_entities(entities))
        })
    }

    /// Append every reviewer override on this group to `out`.
    pub(super) fn collect_overrides_into(&self, out: &mut Vec<(Uuid, ModalityRedactions)>) {
        dispatch!(self, |_M, entities| extend_overrides(out, entities))
    }

    /// Merge post-apply provenance from `report`'s body into
    /// this group's records, matched by entity id.
    ///
    /// After [`Orchestrator::anonymize_with`] applies operators,
    /// each mutated `Entity<M>` on the returned report carries a
    /// fresh redaction event appended to its
    /// `provenance.events`. This helper moves that chain onto
    /// the caller's [`EntityRecord<M>`] so the audit is visible
    /// via `audit.body`.
    ///
    /// [`Orchestrator::anonymize_with`]: elide::Orchestrator::anonymize_with
    pub(super) fn merge_body_from(&mut self, report: &mut Report) {
        dispatch!(self, |M, entities| {
            merge_provenance::<M>(entities, |f| report.for_each_body_mut::<M>(f));
        })
    }

    /// Merge post-apply provenance for a container part. Same
    /// shape as [`Self::merge_body_from`], keyed by part id.
    pub(super) fn merge_part_from(&mut self, report: &mut Report, id: &str) {
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
    pub(super) fn encode_redacted_from(&self, handle: UntypedDocumentHandle) -> Result<Bytes> {
        dispatch!(self, |M, _entities| encode_typed::<M>(handle))
    }
}

fn clone_entities<M: Modality>(records: &[EntityRecord<M>]) -> Vec<Entity<M>>
where
    Entity<M>: Clone,
{
    records.iter().map(|r| r.entity.clone()).collect()
}

fn extend_overrides<M: Modality>(
    out: &mut Vec<(Uuid, ModalityRedactions)>,
    records: &[EntityRecord<M>],
) {
    out.extend(
        records
            .iter()
            .filter_map(|r| r.review.as_ref().map(|a| (r.entity.id, a.clone()))),
    );
}

/// Index `records` by id, then let `walk_mutated` invoke the
/// per-entity callback once per mutated `Entity<M>`. For each
/// mutated entity whose id matches a record, `mem::take` moves
/// its provenance chain onto the record. O(n + m) with one
/// HashMap allocation, no per-mutated linear scan.
fn merge_provenance<M: Modality>(
    records: &mut [EntityRecord<M>],
    walk_mutated: impl FnOnce(&mut dyn FnMut(&mut Entity<M>)),
) {
    let mut by_id: HashMap<Uuid, &mut EntityRecord<M>> =
        records.iter_mut().map(|r| (r.entity.id, r)).collect();
    walk_mutated(&mut |entity| {
        if let Some(record) = by_id.get_mut(&entity.id) {
            record.entity.provenance = mem::take(&mut entity.provenance);
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
