//! Projecting reviewer edits into the shape the provider applies.
//!
//! [`Edit::Redact`] is the one edit that does not land on the
//! report: it names an operator for the anonymizer to run instead of
//! the one the policy set would have picked. `elide-review` holds
//! the reviewer's vocabulary and `elide-provider` holds the
//! anonymizer's, and neither depends on the other — so the
//! translation between them lives here, in the crate that composes
//! the two.

use elide_governance::modality::RedactableModality;
use elide_provider::{Override, Overrides};
use elide_review::{Edit, EditSet};

/// The operator overrides a reviewer asked for, ready to attach.
///
/// Only [`Edit::Redact`] yields one: a suppression names no operator
/// and is applied by stamping the entity itself, an add and a retag
/// hand their entity to the policy set unchanged. The provider never
/// learns about those, because none of them changes which operator
/// it should compile.
#[must_use]
pub fn overrides(edits: &EditSet) -> Overrides {
    Overrides {
        text: overrides_for(&edits.text),
        tabular: overrides_for(&edits.tabular),
        image: overrides_for(&edits.image),
        audio: overrides_for(&edits.audio),
    }
}

/// The operator overrides among one modality's edits, in order.
fn overrides_for<M: RedactableModality>(edits: &[Edit<M>]) -> Vec<Override<M>> {
    edits
        .iter()
        .filter_map(|edit| match edit {
            Edit::Redact(e) => Some(Override {
                entity_id: e.id,
                policy_id: e.policy_id,
                action: e.action.clone(),
            }),
            Edit::Add(_) | Edit::Suppress(_) | Edit::Retag(_) => None,
        })
        .collect()
}
