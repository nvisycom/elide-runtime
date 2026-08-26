//! Reviewer decisions over elide's detected entities.
//!
//! Detections themselves live in elide's [`Report`], inside the
//! [`Audit`]. The layer elide does not model — a reviewer's
//! [`Edit`]s — lives in `elide-review` and is re-exported here, so
//! a caller reads a report and edits it through one crate.
//!
//! Re-exports elide's own entity vocabulary — [`Entity`],
//! [`Label`], [`LabelCatalog`], the audit-trail types — so a caller
//! reads a report without reaching past this crate.
//!
//! [`Audit`]: crate::Audit
//! [`Report`]: elide::Report

pub use elide::entity::audit::{
    Attribution, AuditEvent, AuditKind, AuditLog, ModelEvent, PatternEvent, RuleMatch,
};
pub use elide::entity::{
    Entity, EntityCoRef, EntityRef, Label, LabelCatalog, LabelLocale, LabelRef,
};
#[doc(inline)]
pub use elide_review::{Add, Edit, EditBucket, EditError, EditSet, Retag, Reviewer, Suppress};
