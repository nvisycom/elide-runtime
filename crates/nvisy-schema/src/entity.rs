//! Detected entities, their labels, and their audit trail.
//!
//! [`Entity`] is the modality-generic detection record produced
//! by the analyzer and consumed by the anonymizer. [`Label`] and
//! [`LabelRef`] name the entity kind; [`LabelCatalog`] holds the
//! deployment's label vocabulary. Each label carries per-language
//! [`LabelLocale`] entries (name + optional description); NER
//! and LLM backends render these in the analysis language.
//! [`Provenance`] carries the audit trail (which rule / model /
//! pattern produced each detection).

pub use elide_core::entity::provenance::{
    Attribution, Event, EventKind, ModelEvent, PatternEvent, Provenance, RuleMatch,
};
pub use elide_core::entity::{
    Entity, EntityCoRef, EntityRef, Label, LabelCatalog, LabelLocale, LabelRef,
};
