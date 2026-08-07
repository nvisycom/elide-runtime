//! Detected entities, their labels, and their audit trail.
//!
//! [`Entity`] is the modality-generic detection record produced
//! by the analyzer and consumed by the anonymizer; [`EntityRecord`]
//! is the engine's wrapper that pairs one entity with an optional
//! reviewer override for the apply-time re-decision.
//! [`EntityGroup`] wraps a whole `Vec` of records, tagged by
//! modality, as the unit an [`Audit`] holds in `body` and every
//! `parts` entry. [`Label`] and [`LabelRef`] name the entity
//! kind; each label carries per-language [`LabelLocale`] entries
//! (name + optional description) so NER and LLM backends render
//! labels in the analysis language. [`LabelCatalog`] holds the
//! deployment's label vocabulary and exposes tag-filter helpers
//! ([`LabelCatalog::tagged`], [`LabelCatalog::refs_tagged`],
//! [`LabelCatalog::filter_tag`]) — the primitive regulatory policy
//! templates compose on top of to resolve `tags: ["phi"]`-style
//! selectors into concrete label sets. [`Provenance`] carries
//! the audit trail (which rule / model / pattern produced each
//! detection).
//!
//! [`Audit`]: crate::Audit
//! [`LabelCatalog::tagged`]: elide_core::entity::LabelCatalog::tagged
//! [`LabelCatalog::refs_tagged`]: elide_core::entity::LabelCatalog::refs_tagged
//! [`LabelCatalog::filter_tag`]: elide_core::entity::LabelCatalog::filter_tag

mod group;
mod record;

pub use nvisy_schema::entity::{
    Attribution, Entity, EntityCoRef, EntityRef, Event, EventKind, Label, LabelCatalog,
    LabelLocale, LabelRef, ModelEvent, PatternEvent, Provenance, RuleMatch,
};

pub use self::group::EntityGroup;
pub(crate) use self::group::{take_body, take_part};
pub use self::record::EntityRecord;
