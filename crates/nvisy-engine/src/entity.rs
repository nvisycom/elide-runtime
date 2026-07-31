//! Detected entities and the leaf types their fields carry.
//!
//! [`Entity`] is the modality-generic detection record produced
//! by the analyzer and consumed by the anonymizer; [`EntityRecord`]
//! is the engine's wrapper that pairs one entity with an optional
//! reviewer override for the apply-time re-decision. [`Label`]
//! and [`LabelRef`] name the entity kind; [`LabelCatalog`] holds
//! the deployment's label vocabulary. [`Provenance`] carries the
//! audit trail (which rule / model / pattern produced each
//! detection).
//!
//! Primitive value types ([`BoundingBox`], [`Confidence`],
//! [`Language`], ...) appear as leaf fields on entities and on
//! wire schemas that constrain detection.

pub use nvisy_schema::entity::{
    Attribution, Entity, EntityCoRef, EntityRef, Event, EventKind, Label, LabelCatalog, LabelRef,
    ModelEvent, PatternEvent, Provenance, RuleMatch,
};
pub use nvisy_schema::primitive::{
    BoundingBox, Color, Confidence, ConfidenceThreshold, CountryCode, Dimensions, Dpi, Language,
    LanguageProvenance, LanguageSpan, LanguageTag, Languages, PixelRegion, Point, Polygon,
    TimeSpan, UnitBoundingBox,
};

pub use crate::pipeline::EntityRecord;
