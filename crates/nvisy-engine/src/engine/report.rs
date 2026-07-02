//! Plumbing between the persistence-shaped [`DocBody`] /
//! [`RecognizedGroup`] and elide's runtime [`Report`].
//!
//! Two directions:
//!
//! - **Drain**: after [`Orchestrator::analyze`] returns a
//!   [`Report`], [`take_body`] and [`take_part`] move each typed
//!   `Vec<Entity<M>>` out of the report into the matching
//!   [`RecognizedGroup`] variant for persistence.
//! - **Rebuild**: at apply time, [`insert_body`] and
//!   [`insert_part`] feed a fresh [`Report`] from the persisted
//!   groups (cloning entities — the persisted body is the source
//!   of truth for re-apply idempotency).
//!
//! Plus two byte-level helpers used at the apply seam:
//! [`collect_overrides_into`] walks reviewer overrides off any
//! group, and [`encode_redacted`] picks the right typed handle
//! to re-encode through after `anonymize_with` mutated the
//! document in place.
//!
//! All helpers are stateless — no [`Engine`] state, no I/O. The
//! per-modality dispatch is collapsed onto one trait
//! ([`GroupCarrier`]) plus four trivial impls so adding a fifth
//! modality is one macro line, not eight functions.
//!
//! [`DocBody`]: crate::runs::DocBody
//! [`Engine`]: super::Engine
//! [`Orchestrator::analyze`]: elide::Orchestrator::analyze
//! [`Report`]: elide::Report

use std::mem;

use elide::codec::{PartId, UntypedDocumentHandle};
use elide_core::entity::Entity;
use elide_core::modality::Modality;
use elide_core::modality::audio::Audio;
use elide_core::modality::image::Image;
use elide_core::modality::tabular::Tabular;
use elide_core::modality::text::Text;
use nvisy_core::{Error, Result};
use nvisy_schema::policy::RuleAction;
use uuid::Uuid;

use super::ApplyOutcome;
use crate::runs::{EntityRecord, RecognizedGroup};

const COMPONENT: &str = "engine::report";

/// Per-modality bridge between `Vec<Entity<M>>` and the matching
/// [`RecognizedGroup`] variant. Implemented for each of the four
/// modalities so the drain helpers below can be generic over `M`.
pub(super) trait GroupCarrier: Modality + Sized + 'static {
    /// Wrap a drained `Vec<Entity<Self>>` into the matching
    /// `RecognizedGroup` variant.
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
impl_group_carrier!(Tabular, Tabular);
impl_group_carrier!(Image, Image);
impl_group_carrier!(Audio, Audio);

/// Drain the body's entities from `report` into a
/// [`RecognizedGroup`] of `M`'s variant, or `None` if the body
/// is a different modality.
pub(super) fn take_body<M: GroupCarrier>(report: &mut elide::Report) -> Option<RecognizedGroup> {
    let entities = mem::take(report.entities::<M>()?);
    Some(M::into_group(entities))
}

/// Drain the part `id`'s entities into a [`RecognizedGroup`] of
/// `M`'s variant, or `None` if `M` is not that part's modality.
pub(super) fn take_part<M: GroupCarrier>(
    report: &mut elide::Report,
    id: &PartId,
) -> Option<RecognizedGroup> {
    let entities = mem::take(report.part_entities::<M>(id)?);
    Some(M::into_group(entities))
}

/// Insert the body group into `report` under its modality.
pub(super) fn insert_body(report: elide::Report, group: &RecognizedGroup) -> elide::Report {
    match group {
        RecognizedGroup::Text { entities } => report.insert_body::<Text>(clone_entities(entities)),
        RecognizedGroup::Tabular { entities } => {
            report.insert_body::<Tabular>(clone_entities(entities))
        }
        RecognizedGroup::Image { entities } => {
            report.insert_body::<Image>(clone_entities(entities))
        }
        RecognizedGroup::Audio { entities } => {
            report.insert_body::<Audio>(clone_entities(entities))
        }
    }
}

/// Insert one part group into `report` under its modality.
pub(super) fn insert_part(
    report: elide::Report,
    id: &str,
    group: &RecognizedGroup,
) -> elide::Report {
    let part_id = PartId::from(id.to_owned());
    match group {
        RecognizedGroup::Text { entities } => {
            report.insert_part::<Text>(part_id, clone_entities(entities))
        }
        RecognizedGroup::Tabular { entities } => {
            report.insert_part::<Tabular>(part_id, clone_entities(entities))
        }
        RecognizedGroup::Image { entities } => {
            report.insert_part::<Image>(part_id, clone_entities(entities))
        }
        RecognizedGroup::Audio { entities } => {
            report.insert_part::<Audio>(part_id, clone_entities(entities))
        }
    }
}

fn clone_entities<M: Modality>(records: &[EntityRecord<M>]) -> Vec<Entity<M>>
where
    Entity<M>: Clone,
{
    records.iter().map(|r| r.entity.clone()).collect()
}

/// Append every reviewer override on `group` to `out`. Iterates
/// the variant-appropriate `Vec<EntityRecord<M>>` and keeps only
/// records whose `override` field is set.
pub(super) fn collect_overrides_into(out: &mut Vec<(Uuid, RuleAction)>, group: &RecognizedGroup) {
    match group {
        RecognizedGroup::Text { entities } => extend_overrides(out, entities),
        RecognizedGroup::Tabular { entities } => extend_overrides(out, entities),
        RecognizedGroup::Image { entities } => extend_overrides(out, entities),
        RecognizedGroup::Audio { entities } => extend_overrides(out, entities),
    }
}

fn extend_overrides<M: Modality>(out: &mut Vec<(Uuid, RuleAction)>, records: &[EntityRecord<M>]) {
    out.extend(
        records
            .iter()
            .filter_map(|r| r.r#override.as_ref().map(|a| (r.entity.id, a.clone()))),
    );
}

/// After `anonymize_with` mutated `handle` in place, recover the
/// typed handle for the doc's body modality and re-encode it.
/// `handle` was a typed `DocumentHandle<M>` before being erased;
/// the apply-time re-encode needs the typed form because
/// [`elide::codec::DocumentHandle::encode`] is per-modality.
pub(super) fn encode_redacted(
    handle: UntypedDocumentHandle,
    body: &RecognizedGroup,
) -> Result<ApplyOutcome> {
    match body {
        RecognizedGroup::Text { .. } => encode_typed::<Text>(handle, "Text"),
        RecognizedGroup::Tabular { .. } => encode_typed::<Tabular>(handle, "Tabular"),
        RecognizedGroup::Image { .. } => encode_typed::<Image>(handle, "Image"),
        RecognizedGroup::Audio { .. } => encode_typed::<Audio>(handle, "Audio"),
    }
}

fn encode_typed<M>(handle: UntypedDocumentHandle, name: &'static str) -> Result<ApplyOutcome>
where
    M: Modality,
{
    let typed = handle.into::<M>().map_err(|_| {
        Error::internal(
            format!(
                "post-apply re-encode: handle is not {name} — orchestrator \
                 returned a handle of a different modality than analyze \
                 recorded"
            ),
            COMPONENT,
        )
    })?;
    let content = typed
        .encode()
        .map_err(|err| Error::internal("post-apply encode failed", COMPONENT).with_source(err))?;
    Ok(ApplyOutcome {
        bytes: content.into_bytes(),
    })
}
