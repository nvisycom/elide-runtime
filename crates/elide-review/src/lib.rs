#![forbid(unsafe_code)]
#![cfg_attr(docsrs, feature(doc_cfg))]
#![doc = include_str!("../README.md")]

//! Reviewer edits over an elide report.
//!
//! An analyzed document goes to a human before it is redacted, and
//! what they decide lands here: a detection recognition missed, a
//! wrong label, a false positive, an operator the policy set would
//! not have picked.
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
//! [`Report`]. Three of the four reach the document that way. The
//! operator override does not: it names an operator for the
//! anonymizer to run, so a consumer projects it onto whatever it
//! drives — elide re-resolves operators from live policy at apply
//! time and has no per-entity override of its own.
//!
//! ## Layout
//!
//! - `edit` is the data: the four operations, and the rules for
//!   when two of them compose rather than contradict.
//! - `apply` is the other half — landing an edit on a report, the
//!   suppression flag elide actually reads, and finding an entity
//!   across a report's body and its container parts.
//!
//! [`Report`]: elide::Report
//! [`validate`]: EditSet::validate

mod apply;
mod edit;

pub use self::edit::{Add, Edit, EditBucket, EditSet, Redact, Retag, Reviewer, Suppress};
