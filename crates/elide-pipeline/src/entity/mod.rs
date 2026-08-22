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
//! [`LabelCatalog::filter_tag`]): the primitive regulatory policy
//! templates compose on top of to resolve `tags: ["phi"]`-style
//! selectors into concrete label sets. [`AuditLog`] carries
//! the audit trail (which rule / model / pattern produced each
//! detection).
//!
//! [`Audit`]: crate::Audit
//! [`LabelCatalog::tagged`]: elide::entity::LabelCatalog::tagged
//! [`LabelCatalog::refs_tagged`]: elide::entity::LabelCatalog::refs_tagged
//! [`LabelCatalog::filter_tag`]: elide::entity::LabelCatalog::filter_tag

mod group;
mod overrides;
mod record;

pub use elide::entity::audit::{
    Attribution, AuditEvent, AuditKind, AuditLog, ModelEvent, PatternEvent, RuleMatch,
};
pub use elide::entity::{
    Entity, EntityCoRef, EntityRef, Label, LabelCatalog, LabelLocale, LabelRef,
};

pub use self::group::EntityGroup;
pub(crate) use self::group::{take_body, take_part};
pub(crate) use self::overrides::OverrideSet;
pub use self::record::{EntityRecord, OverrideEntry, Review};
