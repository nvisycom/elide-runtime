//! [`HiddenContentPlan`]: which flagged hidden content a pass removes.
//!
//! Decode makes every metadata field and collaboration item its
//! format labels an entity, under the hidden-content policy the
//! runtime decodes with. Each one is then either removed or kept,
//! never masked or replaced. An item is removed when:
//!
//! - **a reviewer removed it**: its latest verdict is a `Confirm`,
//!   which is what a reviewer's remove, and remove-all, record;
//! - **its label is sensitive**: the request's catalog declares the
//!   label the format gave it (an EXIF position is a location, a
//!   document's `creator` a person);
//! - **its value is sensitive**: a live finding, a recognizer's or a
//!   reviewer's, lies inside it or across its edge, an address in a
//!   description or a name in a comment.
//!
//! Everything else is kept, and so is an item a reviewer chose to
//! keep: its latest verdict is a `Suppress`. The item's own trail
//! decides, so when reviews disagree over time the latest one wins.
//!
//! A reviewer's keep suppresses everything within the item at the
//! time, which elide's redaction pass skips. A span a reviewer adds
//! inside a kept item afterwards is its own decision: it is redacted
//! like any finding, and the rest of the item is kept. Removing the
//! whole item takes a remove on the item itself. A removed item's
//! `Remove` covers whatever lies inside it.
//!
//! Worked out over the whole report rather than per entity because a
//! value's sensitivity is about two entities at once: a finding and
//! the item containing it. A rule only ever sees one.
//!
//! Only text and metadata streams flag items, so only their
//! anonymizers carry the plan's rules.

use std::any::Any;
use std::collections::HashSet;
use std::sync::Arc;

use elide::entity::audit::{Attribution, AuditKind, ManualIntent};
use elide::entity::{Entity, LabelCatalog};
use elide::modality::metadata::MetadataLocation;
use elide::modality::{Modality, ModalityLocation, Overlap};
use elide::redaction::operators::{Keep, Remove};
use elide::redaction::{HiddenContentKind, Operator, Rule};
use elide::{PartId, Report};
use schemars::JsonSchema;
use serde::Serialize;
use uuid::Uuid;

use crate::index::HiddenItems;

/// Why a hidden-content item is removed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Removal {
    /// The request's catalog declares the label its format gave it.
    Label,
    /// A live finding, a recognizer's or a reviewer's, lies inside its
    /// value.
    Value,
    /// A reviewer removed it.
    Reviewer,
}

/// One hidden-content item, and what a pass does with it.
#[derive(Debug, Clone, PartialEq, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct HiddenItem {
    /// The item's entity id: what a reviewer's decision names.
    pub id: Uuid,
    /// The part holding it, top-level document first.
    pub part: Vec<String>,
    /// The modality it was decoded as: `metadata` for a field,
    /// `text` for a comment or tracked change.
    pub modality: &'static str,
    /// What kind of hidden content it is.
    #[schemars(with = "String")]
    pub content: HiddenContentKind,
    /// The label its format gave it.
    pub label: String,
    /// The field's key, for a metadata field (`creator`,
    /// `GPSLatitude`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub field: Option<String>,
    /// Why it is removed, or `None` when it is kept.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub removal: Option<Removal>,
    /// Whether a reviewer chose to keep it, overriding a removal.
    pub kept_by_reviewer: bool,
}

/// The hidden content of a report, and which of it a pass removes.
#[derive(Debug, Clone, Default)]
pub struct HiddenContentPlan {
    items: Vec<HiddenItem>,
    /// The flagged items the pass removes.
    removed: Arc<HashSet<Uuid>>,
}

impl HiddenContentPlan {
    /// Work out the plan over every part of `report`, against the
    /// request's `catalog`.
    #[must_use]
    pub fn new(report: &Report, catalog: &LabelCatalog) -> Self {
        let index = HiddenItems::of(report);
        let mut plan = Planning::default();
        let mut refs = index.iter().peekable();
        // The index runs part by part, so each part's items are
        // planned together, against that part's entities.
        while let Some(first) = refs.next() {
            let mut group = vec![first.id];
            while let Some(next) = refs.next_if(|next| next.part == first.part) {
                group.push(next.id);
            }
            elide_schema::each_modality!(|M| {
                if <M as Modality>::NAME == first.modality {
                    plan.part::<M>(report, &first.part, &group, catalog);
                }
            });
        }
        plan.finish()
    }

    /// Every hidden-content item, kept or removed, in part order.
    #[must_use]
    pub fn items(&self) -> &[HiddenItem] {
        &self.items
    }

    /// Consume the plan for its items.
    #[must_use]
    pub fn into_items(self) -> Vec<HiddenItem> {
        self.items
    }

    /// The rules every text and metadata anonymizer starts with:
    /// remove the flagged items this plan removes, and keep the rest.
    ///
    /// First in the list, so a policy rule naming the same label
    /// never reaches a flagged item: hidden content is removed or
    /// kept, never masked or replaced.
    pub fn rules<M>(&self) -> [Rule<M>; 2]
    where
        M: Modality,
        Remove: Operator<M>,
        Keep: Operator<M>,
    {
        let removed = Arc::clone(&self.removed);
        [
            Rule::predicate(
                move |cx| cx.entity.audit().is_flagged() && removed.contains(&cx.entity.id()),
                Remove,
            )
            .because(Attribution::freeform("hidden-content-removed")),
            Rule::predicate(|cx| cx.entity.audit().is_flagged(), Keep)
                .because(Attribution::freeform("hidden-content-kept")),
        ]
    }
}

/// A plan being worked out, part by part.
#[derive(Default)]
struct Planning {
    items: Vec<HiddenItem>,
    removed: HashSet<Uuid>,
}

impl Planning {
    /// Plan the items `ids` of part `id`, held as modality `M`.
    fn part<M: Modality>(
        &mut self,
        report: &Report,
        id: &PartId,
        ids: &[Uuid],
        catalog: &LabelCatalog,
    ) {
        let Ok(part) = report.part::<M>(id) else {
            return;
        };
        let entities = part.entities();
        let path: Vec<String> = id.segments().map(str::to_owned).collect();
        for item in ids.iter().filter_map(|&item| part.entity(item)) {
            let Some(content) = flagged_as(item) else {
                continue;
            };
            let kept_by_reviewer = item.is_suppressed();
            let removal = (!kept_by_reviewer)
                .then(|| removal(item, entities, catalog))
                .flatten();
            if removal.is_some() {
                self.removed.insert(item.id());
            }
            self.items.push(HiddenItem {
                id: item.id(),
                part: path.clone(),
                modality: M::NAME,
                content,
                label: item.label().as_str().to_owned(),
                field: field_key::<M>(item.location()),
                removal,
                kept_by_reviewer,
            });
        }
    }

    fn finish(self) -> HiddenContentPlan {
        HiddenContentPlan {
            items: self.items,
            removed: Arc::new(self.removed),
        }
    }
}

/// The key of the metadata field at `location`, or `None` for an
/// item of another modality, which has no key.
fn field_key<M: Modality>(location: &M::Location) -> Option<String> {
    let location: &dyn Any = location;
    location
        .downcast_ref::<MetadataLocation>()
        .map(|location| location.key.to_string())
}

/// The kind of hidden content `entity` was flagged as, or `None`
/// when it is a detection.
fn flagged_as<M: Modality>(entity: &Entity<M>) -> Option<HiddenContentKind> {
    entity
        .audit()
        .events()
        .iter()
        .find_map(|event| match &event.kind {
            AuditKind::Flagged(flagged) => Some(flagged.content),
            _ => None,
        })
}

/// Why `item` is removed, given the `entities` of its part, or
/// `None` when it is kept.
fn removal<M: Modality>(
    item: &Entity<M>,
    entities: &[Entity<M>],
    catalog: &LabelCatalog,
) -> Option<Removal> {
    if item.audit().verdict() == Some(&ManualIntent::Confirm) {
        return Some(Removal::Reviewer);
    }
    if catalog.contains(item.label()) {
        return Some(Removal::Label);
    }
    // Any overlap, not only containment: a finding straddling the
    // item's edge still says something sensitive is in it.
    entities
        .iter()
        .any(|entity| {
            !entity.audit().is_flagged()
                && !entity.is_suppressed()
                && item.location().overlap(entity.location()) != Overlap::Disjoint
        })
        .then_some(Removal::Value)
}
