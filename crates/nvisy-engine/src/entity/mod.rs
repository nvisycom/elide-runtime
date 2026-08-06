//! Detected entities, their labels, and their audit trail.
//!
//! [`Entity`] is the modality-generic detection record produced
//! by the analyzer and consumed by the anonymizer; [`EntityRecord`]
//! is the engine's wrapper that pairs one entity with an optional
//! reviewer override for the apply-time re-decision.
//! [`EntityGroup`] wraps a whole `Vec` of records, tagged by
//! modality, as the unit an [`Audit`] holds in `body` and every
//! `parts` entry. [`Label`] and [`LabelRef`] name the entity
//! kind; [`LabelCatalog`] holds the deployment's label vocabulary.
//! [`Provenance`] carries the audit trail (which rule / model /
//! pattern produced each detection).
//!
//! [`Audit`]: crate::Audit

mod group;
mod record;

pub use nvisy_schema::entity::{
    Attribution, Entity, EntityCoRef, EntityRef, Event, EventKind, Label, LabelCatalog, LabelRef,
    ModelEvent, PatternEvent, Provenance, RuleMatch,
};

pub use self::group::EntityGroup;
pub use self::record::EntityRecord;
pub(crate) use self::group::{take_body, take_part};
