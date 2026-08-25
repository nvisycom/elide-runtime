//! Finding an entity anywhere in a report.

use elide::Report;
use elide::codec::PartId;
use elide::entity::Entity;
use elide::modality::Modality;
use uuid::Uuid;

/// Find an entity by id anywhere in the report: the body first,
/// then every container part.
///
/// A report indexes entities per location, not globally, so a
/// decision keyed only by entity id has to be looked for.
pub(super) fn entity_mut<M: Modality>(report: &mut Report, id: Uuid) -> Option<&mut Entity<M>> {
    if report.entity_mut::<M>(id).is_some() {
        return report.entity_mut::<M>(id);
    }
    let part_ids: Vec<PartId> = report.part_ids().map(|(id, _)| id.clone()).collect();
    part_ids
        .into_iter()
        .find(|part| report.part_entity_mut::<M>(part, id).is_some())
        .and_then(move |part| report.part_entity_mut::<M>(&part, id))
}
