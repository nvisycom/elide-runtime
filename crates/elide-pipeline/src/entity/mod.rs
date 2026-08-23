//! Reviewer decisions over elide's detected entities.
//!
//! Detections themselves live in elide's [`Report`], inside the
//! [`Audit`]. What sits here is the layer elide does not model: a
//! [`Review`] per entity, collected in a [`ReviewSet`] keyed by
//! entity id, and the [`OverrideEntry`] it compiles into for the
//! anonymize path.
//!
//! Re-exports elide's own entity vocabulary — [`Entity`],
//! [`Label`], [`LabelCatalog`], the audit-trail types — so a caller
//! reads a report without reaching past this crate.
//!
//! [`Audit`]: crate::Audit
//! [`Report`]: elide::Report

mod apply;
mod overrides;
mod record;
mod reviews;

pub use elide::entity::audit::{
    Attribution, AuditEvent, AuditKind, AuditLog, ModelEvent, PatternEvent, RuleMatch,
};
pub use elide::entity::{
    Entity, EntityCoRef, EntityRef, Label, LabelCatalog, LabelLocale, LabelRef,
};

pub(crate) use self::overrides::OverrideSet;
pub use self::record::{OverrideEntry, Review};
pub use self::reviews::{ReviewBucket, ReviewSet};
