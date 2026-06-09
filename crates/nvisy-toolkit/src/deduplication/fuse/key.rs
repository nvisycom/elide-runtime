//! [`GroupKey`]: hash key for the first grouping phase.

use nvisy_core::entity::{Entity, EntityKind};
use nvisy_core::extraction::TextAt;
use nvisy_core::modality::Modality;

use super::group::GroupingCriteria;

/// Hash key for the first grouping phase.
///
/// For [`Strict`], [`Narrowing`], and [`Widening`] this stores the
/// exact value; for [`Normalized`] it stores the lowercased,
/// trimmed form.
///
/// [`Strict`]: GroupingCriteria::Strict
/// [`Narrowing`]: GroupingCriteria::Narrowing
/// [`Widening`]: GroupingCriteria::Widening
/// [`Normalized`]: GroupingCriteria::Normalized
#[derive(Hash, PartialEq, Eq)]
pub(super) struct GroupKey {
    pub(super) kind: EntityKind,
    pub(super) value: String,
}

impl GroupKey {
    pub(super) async fn new<M: Modality, V: TextAt<M> + ?Sized>(
        entity: &Entity<M>,
        criteria: GroupingCriteria,
        view: &V,
    ) -> Self {
        // Entities without a text value (e.g. image bounding boxes)
        // get a unique sentinel so they don't all bucket together.
        // They will still be grouped by location overlap in phase 2.
        let value = match view.text_at(&entity.location).await {
            Some(v) => criteria.bucket_value(&v),
            None => entity.id.to_string(),
        };
        Self {
            kind: entity.entity_kind,
            value,
        }
    }
}
