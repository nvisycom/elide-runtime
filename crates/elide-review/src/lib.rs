#![forbid(unsafe_code)]
#![cfg_attr(docsrs, feature(doc_cfg))]
#![doc = include_str!("../README.md")]

//! Reviewer edits over an elide report.
//!
//! An analyzed document goes to a human before it is redacted, and
//! what they decide lands here: a detection recognition missed, a
//! wrong label, a false positive, and which hidden content (metadata
//! fields, comments, tracked changes) goes.
//!
//! # Two layers
//!
//! [`Edit`] and [`EditSet`] are plain data — deserialize them from
//! a request body, [`validate`] them, hand them on. Nothing in that
//! path needs an engine, which is why this is its own crate: an
//! HTTP layer can accept and check a reviewer's edits before it has
//! anything to apply them to.
//!
//! [`EditSet::apply`] is the other half, landing those edits on a
//! [`Report`]. Every decision reaches the document that way, so a caller
//! that has applied an edit set needs nothing further from it.
//!
//! # Hidden content
//!
//! Hidden content is review end to end, so it lives here whole:
//! [`HiddenItems`] says which entities are items, [`HiddenContentEdits`]
//! what a reviewer decided about them, and [`HiddenContentPlan`] what
//! a pass therefore removes, both as the preview a reviewer sees and
//! as the rules an anonymizer starts with.
//!
//! ## Layout
//!
//! - `edit` is the data: the three operations, the hidden-content
//!   decisions, and the rules for when two of them compose rather
//!   than contradict.
//! - `apply` is the other half — landing an edit on a report, the
//!   suppression flag elide actually reads, and finding an entity
//!   across a report's body and its container parts.
//! - `index` finds every hidden-content item, in part order.
//! - `plan` decides which items a pass removes, and why.
//!
//! [`Report`]: elide::Report
//! [`validate`]: EditSet::validate

mod apply;
mod edit;
mod index;
mod plan;

pub use self::edit::{
    Add, Edit, EditBucket, EditError, EditSet, HiddenChoice, HiddenContentEdits, RemoveAll, Retag,
    Suppress,
};
pub use self::index::{HiddenItems, HiddenRef, sorted_part_ids};
pub use self::plan::{HiddenContentPlan, HiddenItem, Removal};
