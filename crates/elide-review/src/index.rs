//! [`HiddenItems`]: every hidden-content item a report holds, and
//! where.
//!
//! An item is an entity whose trail records its format flagging it
//! (a metadata field, a comment, its author, a tracked change). That
//! definition lives here alone: the runtime's hidden-content plan,
//! edit validation, and review recording all read items through this
//! index, so they cannot disagree about which ids are items.

use std::collections::HashMap;

use elide::modality::Modality;
use elide::{PartId, Report};
use uuid::Uuid;

/// One hidden-content item: its entity id, and the part and modality
/// holding it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HiddenRef {
    /// The item's entity id.
    pub id: Uuid,
    /// The part holding it.
    pub part: PartId,
    /// The modality it was decoded as ([`Modality::NAME`]).
    pub modality: &'static str,
}

/// Every hidden-content item of a report, in part order, each
/// part's in entity order.
#[derive(Debug, Clone, Default)]
pub struct HiddenItems {
    items: Vec<HiddenRef>,
    by_id: HashMap<Uuid, usize>,
}

impl HiddenItems {
    /// Index the hidden-content items of `report`, in one walk.
    #[must_use]
    pub fn of(report: &Report) -> Self {
        let mut items = Vec::new();
        for part in sorted_part_ids(report) {
            elide_schema::each_modality!(|M| collect::<M>(report, part, &mut items));
        }
        let by_id = items
            .iter()
            .enumerate()
            .map(|(index, item)| (item.id, index))
            .collect();
        Self { items, by_id }
    }

    /// The item `id`, or `None` when `id` names no hidden-content
    /// item.
    #[must_use]
    pub fn get(&self, id: Uuid) -> Option<&HiddenRef> {
        self.by_id.get(&id).map(|&index| &self.items[index])
    }

    /// Whether `id` names a hidden-content item.
    #[must_use]
    pub fn contains(&self, id: Uuid) -> bool {
        self.by_id.contains_key(&id)
    }

    /// Every item, in part order.
    pub fn iter(&self) -> impl Iterator<Item = &HiddenRef> {
        self.items.iter()
    }

    /// Whether the report holds no hidden content.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }
}

/// The part ids of `report` by path, so a document sorts before the
/// parts nested under it, and a walk over them is the same on every
/// run: the report keeps its parts unordered.
#[must_use]
pub fn sorted_part_ids(report: &Report) -> Vec<&PartId> {
    let mut ids: Vec<&PartId> = report.part_ids().map(|(id, _)| id).collect();
    ids.sort_by(|a, b| a.segments().cmp(b.segments()));
    ids
}

/// Add the flagged entities of `part`, if it holds modality `M`.
fn collect<M: Modality>(report: &Report, part: &PartId, out: &mut Vec<HiddenRef>) {
    let Ok(entities) = report.part::<M>(part) else {
        return;
    };
    out.extend(
        entities
            .entities()
            .iter()
            .filter(|entity| entity.audit().is_flagged())
            .map(|entity| HiddenRef {
                id: entity.id(),
                part: part.clone(),
                modality: M::NAME,
            }),
    );
}
