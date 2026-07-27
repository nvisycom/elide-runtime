//! Detected entities, their labels, and their audit trail.
//!
//! [`Entity`] is the modality-generic detection record produced
//! by the analyzer and consumed by the anonymizer. [`Label`] and
//! [`LabelRef`] name the entity kind; [`LabelCatalog`] holds the
//! deployment's label vocabulary. [`Provenance`] carries the
//! audit trail (which rule / model / pattern produced each
//! detection).

pub use elide_core::entity::provenance::{
    Attribution, Event, EventKind, ModelEvent, PatternEvent, Provenance, RuleMatch,
};
pub use elide_core::entity::{Entity, EntityCoRef, EntityRef, Label, LabelCatalog, LabelRef};
