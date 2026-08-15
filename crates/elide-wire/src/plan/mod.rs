//! Authored recognition plan: scope, annotations, and per-request
//! codec knobs.
//!
//! Serialisable description of how to build an analyzer for a
//! request. Symmetric with the [`elide-governance`] crate. Where
//! governance describes redaction (which entities to hide and how),
//! the plan describes recognition (which entities to find and how).
//! Both are pure data; the engine compiles them into elide
//! runtime values at request time.
//!
//! ## Layout
//!
//! - [`AnalyzerParams`]: top-level. Scope, region annotations,
//!   per-request OCR mode.
//! - [`AnyAnnotations`]: per-modality region annotations
//!   (inclusions / exclusions).
//!
//! Caller-asserted scope lives under [`AnalyzerParams::scope`],
//! a [`ScopeParams`] carrying `languages`, `countries`, and
//! elide's own `ScopeMetadata` block (tags, purpose, audience).
//! The engine assembles these (plus a server-minted correlation
//! id and the policy-derived label catalog) into an
//! `elide::recognition::Scope` at compile time.
//!
//! [`elide-governance`]: https://docs.rs/elide-governance

mod analyzer;
mod annotation;

pub use self::analyzer::{AnalyzerParams, ScopeParams, scope_metadata_is_empty};
pub use self::annotation::AnyAnnotations;
