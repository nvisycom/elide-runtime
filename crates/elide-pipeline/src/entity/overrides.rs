//! [`OverrideSet`]: reviewer overrides collected across every
//! modality in a request, kept typed per modality.
//!
//! Overrides are gathered from the audit body plus every container
//! part, and a container's parts need not share one modality: a
//! DOCX carries text alongside embedded images. The collection is
//! therefore heterogeneous, while each [`OverrideEntry<M>`] inside
//! it is pinned to the modality of the entity it targets.
//!
//! One typed `Vec` per modality rather than a single vec of a
//! type-erased entry, so the anonymizer's per-modality assemble
//! path receives exactly the entries it can apply and the
//! modality can never disagree with the redaction spec.

use elide::modality::audio::Audio;
use elide::modality::image::Image;
use elide::modality::tabular::Tabular;
use elide::modality::text::Text;
use elide_governance::modality::RedactableModality;
use uuid::Uuid;

use super::record::OverrideEntry;

/// Reviewer overrides for one anonymize request, bucketed by the
/// modality of the entity each one targets.
#[derive(Debug, Clone, Default)]
pub(crate) struct OverrideSet {
    /// Overrides targeting text entities.
    pub(crate) text: Vec<OverrideEntry<Text>>,
    /// Overrides targeting tabular entities.
    pub(crate) tabular: Vec<OverrideEntry<Tabular>>,
    /// Overrides targeting image entities.
    pub(crate) image: Vec<OverrideEntry<Image>>,
    /// Overrides targeting audio entities.
    pub(crate) audio: Vec<OverrideEntry<Audio>>,
}

impl OverrideSet {
    /// Every override's `(entity_id, policy_id)` pair, across all
    /// modalities.
    ///
    /// Validation only needs the authority a reviewer named, not
    /// the operator they picked, so this stays modality-agnostic
    /// and spares the caller four near-identical loops.
    pub(crate) fn authorities(&self) -> impl Iterator<Item = (Uuid, Uuid)> + '_ {
        authorities_of(&self.text)
            .chain(authorities_of(&self.tabular))
            .chain(authorities_of(&self.image))
            .chain(authorities_of(&self.audio))
    }
}

/// The `(entity_id, policy_id)` pair of every entry in one
/// modality's bucket. A free generic fn rather than a closure:
/// each bucket has a different `M`, and a closure cannot be
/// generic over it.
fn authorities_of<M: RedactableModality>(
    entries: &[OverrideEntry<M>],
) -> impl Iterator<Item = (Uuid, Uuid)> + '_ {
    entries.iter().map(|e| (e.entity_id, e.policy_id))
}
