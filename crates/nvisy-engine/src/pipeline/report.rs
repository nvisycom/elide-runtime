//! Plumbing between [`AnalyzedDocument`] / [`RecognizedGroup`] and
//! elide's runtime [`Report`].
//!
//! Two directions:
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
//!
//! Plus two byte-level helpers used at the anonymize seam:
//! [`RecognizedGroup::collect_overrides_into`] walks reviewer
//! overrides off any group, and
//! [`RecognizedGroup::encode_redacted_from`] picks the right
//! typed handle to re-encode through after `anonymize_with`
//! mutated the document in place.
//!
//! All helpers are stateless: no [`Engine`] state, no I/O. The
//! per-modality dispatch is collapsed onto one trait
//! ([`GroupCarrier`]) plus four trivial impls so adding a fifth
//! modality is one macro line, not eight functions.
//!
//! [`AnalyzedDocument`]: crate::AnalyzedDocument
//! [`RecognizedGroup`]: crate::RecognizedGroup
//! [`Engine`]: super::Engine
//! [`Orchestrator::analyze`]: elide::Orchestrator::analyze
//! [`Report`]: elide::Report

use std::mem;

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
use nvisy_schema::policy::PolicyAction;
use uuid::Uuid;

use super::AnonymizedDocument;
use super::analyzed::{EntityRecord, RecognizedGroup};

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

impl RecognizedGroup {
    /// Insert this group into `report` under its modality, as
    /// the body.
    pub(super) fn insert_into_body(&self, report: Report) -> Report {
        match self {
            Self::Text { entities } => report.insert_body::<Text>(clone_entities(entities)),
            #[cfg(feature = "internal_tabular")]
            Self::Tabular { entities } => report.insert_body::<Tabular>(clone_entities(entities)),
            #[cfg(feature = "internal_image")]
            Self::Image { entities } => report.insert_body::<Image>(clone_entities(entities)),
            #[cfg(feature = "internal_audio")]
            Self::Audio { entities } => report.insert_body::<Audio>(clone_entities(entities)),
        }
    }

    /// Insert this group into `report` under its modality, as a
    /// container part keyed by `id`.
    pub(super) fn insert_as_part(&self, report: Report, id: &str) -> Report {
        let part_id = PartId::from(id.to_owned());
        match self {
            Self::Text { entities } => {
                report.insert_part::<Text>(part_id, clone_entities(entities))
            }
            #[cfg(feature = "internal_tabular")]
            Self::Tabular { entities } => {
                report.insert_part::<Tabular>(part_id, clone_entities(entities))
            }
            #[cfg(feature = "internal_image")]
            Self::Image { entities } => {
                report.insert_part::<Image>(part_id, clone_entities(entities))
            }
            #[cfg(feature = "internal_audio")]
            Self::Audio { entities } => {
                report.insert_part::<Audio>(part_id, clone_entities(entities))
            }
        }
    }

    /// Append every reviewer override on this group to `out`.
    ///
    /// Iterates the variant-appropriate `Vec<EntityRecord<M>>`
    /// and keeps only records whose `reviewer_override` field is
    /// set.
    pub(super) fn collect_overrides_into(&self, out: &mut Vec<(Uuid, PolicyAction)>) {
        match self {
            Self::Text { entities } => extend_overrides(out, entities),
            #[cfg(feature = "internal_tabular")]
            Self::Tabular { entities } => extend_overrides(out, entities),
            #[cfg(feature = "internal_image")]
            Self::Image { entities } => extend_overrides(out, entities),
            #[cfg(feature = "internal_audio")]
            Self::Audio { entities } => extend_overrides(out, entities),
        }
    }

    /// Recover the typed handle for this group's modality (the
    /// document body's modality) and re-encode it into an
    /// [`AnonymizedDocument`].
    ///
    /// Called after `anonymize_with` mutated `handle` in place.
    /// The apply-time re-encode needs the typed form because
    /// [`elide::codec::DocumentHandle::encode`] is per-modality.
    pub(super) fn encode_redacted_from(
        &self,
        handle: UntypedDocumentHandle,
    ) -> Result<AnonymizedDocument> {
        match self {
            Self::Text { .. } => encode_typed::<Text>(handle, "Text"),
            #[cfg(feature = "internal_tabular")]
            Self::Tabular { .. } => encode_typed::<Tabular>(handle, "Tabular"),
            #[cfg(feature = "internal_image")]
            Self::Image { .. } => encode_typed::<Image>(handle, "Image"),
            #[cfg(feature = "internal_audio")]
            Self::Audio { .. } => encode_typed::<Audio>(handle, "Audio"),
        }
    }
}

fn clone_entities<M: Modality>(records: &[EntityRecord<M>]) -> Vec<Entity<M>>
where
    Entity<M>: Clone,
{
    records.iter().map(|r| r.entity.clone()).collect()
}

fn extend_overrides<M: Modality>(out: &mut Vec<(Uuid, PolicyAction)>, records: &[EntityRecord<M>]) {
    out.extend(records.iter().filter_map(|r| {
        r.reviewer_override
            .as_ref()
            .map(|a| (r.entity.id, a.clone()))
    }));
}

fn encode_typed<M>(handle: UntypedDocumentHandle, name: &'static str) -> Result<AnonymizedDocument>
where
    M: Modality,
{
    let typed = handle.into::<M>().map_err(|_| {
        Error::new(
            ErrorKind::Redaction,
            format!(
                "post-apply re-encode: handle is not {name} — orchestrator \
                 returned a handle of a different modality than analyze \
                 recorded"
            ),
        )
    })?;
    let content = typed.encode().map_err(|err| {
        Error::new(
            ErrorKind::Redaction,
            format!("post-apply encode failed: {err}"),
        )
    })?;
    Ok(AnonymizedDocument {
        bytes: content.into_bytes(),
    })
}
