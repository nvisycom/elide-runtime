//! Recording [`HiddenContentEdits`] in a review.
//!
//! A remove (and each item a remove-all reaches) confirms the item,
//! which the hidden-content plan reads as a reviewer's removal. A
//! keep suppresses everything within the item, the item and whatever
//! lies inside it, which elide's redaction pass skips. A span added
//! inside the item later is not covered by the keep: it is redacted
//! like any finding.
//!
//! Confirm and suppress are opposite verdicts on the item's own
//! trail, so when reviews disagree over time, the latest decision
//! about an item wins.

use std::collections::HashMap;

use elide::modality::Modality;
use elide::{Report, Review};
use serde::Serialize;
use serde::de::DeserializeOwned;
use uuid::Uuid;

use super::open;
use crate::index::{HiddenItems, HiddenRef};
use crate::{EditError, HiddenContentEdits};

/// What a reviewer decided about one item.
#[derive(Clone, Copy)]
enum Decision {
    Remove,
    Keep,
}

/// Add every decision in `edits` to `review`.
///
/// Decisions are gathered by id first, so the review records them
/// in item order rather than in the order they were sent.
pub(super) fn record(
    edits: &HiddenContentEdits,
    report: &Report,
    items: &HiddenItems,
    review: &mut Review,
) -> Result<(), EditError> {
    let mut decided: HashMap<Uuid, (Decision, Option<&str>)> = HashMap::new();
    match edits {
        HiddenContentEdits::RemoveAll(all) => {
            let reason = all.reason.as_deref();
            for item in items.iter() {
                decided.insert(item.id, (Decision::Remove, reason));
            }
        }
        HiddenContentEdits::Choose(choices) => {
            for choice in &choices.remove {
                decided.insert(choice.id, (Decision::Remove, choice.reason.as_deref()));
            }
            for choice in &choices.keep {
                decided.insert(choice.id, (Decision::Keep, choice.reason.as_deref()));
            }
        }
    }

    // In item order, so the review reads the way the report does.
    for item in items.iter() {
        let Some(&(decision, reason)) = decided.get(&item.id) else {
            continue;
        };
        let mut recorded = Ok(());
        elide_schema::each_modality!(|M| {
            if <M as Modality>::NAME == item.modality {
                recorded = decide::<M>(report, review, item, decision, reason);
            }
        });
        recorded?;
    }
    Ok(())
}

/// Record `decision` about `item`, held as modality `M`.
fn decide<M: Modality>(
    report: &Report,
    review: &mut Review,
    item: &HiddenRef,
    decision: Decision,
    reason: Option<&str>,
) -> Result<(), EditError>
where
    M::Location: Serialize + DeserializeOwned,
{
    let part = report.part::<M>(&item.part)?;
    // The index was built from this report, so the entity is there.
    let Some(entity) = part.entity(item.id) else {
        return Ok(());
    };
    let mut view = open::<M>(review, &item.part, reason)?;
    match decision {
        Decision::Remove => {
            view.confirm(item.id);
        }
        Decision::Keep => {
            view.suppress_within(entity.location().clone());
        }
    }
    Ok(())
}
