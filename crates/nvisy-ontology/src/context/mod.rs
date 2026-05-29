//! Persistent reference-data collections for detection.
//!
//! A [`Context`] holds reusable reference data — names, faces, voices,
//! patterns, embeddings — that tells detection *what to look for*.  It is
//! separate from policy (which controls *what to do* when something is found).
//!
//! ## Current status
//!
//! Today this subtree is **schema-only**. The ontology models the shape
//! [`Context`]s would have (analytic / biometric / document / geospatial
//! / reference / temporal entry kinds), but no detection backend in the
//! workspace currently consumes a `Context` — recognizers receive their
//! reference data through backend-specific configuration. The types
//! exist so persisted contexts round-trip cleanly through the engine
//! and so future backends have a stable target to build against.
//! When a recognizer starts matching on a specific variant, that's the
//! cue to evaluate whether the shape still fits and prune what doesn't.

pub mod analytic;
pub mod biometric;
pub mod document;
mod entry;
pub mod geospatial;
pub mod reference;
pub mod temporal;

use derive_builder::Builder;
use schemars::JsonSchema;
use semver::Version;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

pub use self::entry::{ContextEntry, ContextEntryData};

/// A persistent, reusable collection of reference data for detection.
#[derive(Debug, Clone, Builder, Serialize, Deserialize, JsonSchema)]
#[builder(
    name = "ContextBuilder",
    pattern = "owned",
    setter(into, strip_option, prefix = "with")
)]
#[serde(rename_all = "camelCase")]
pub struct Context {
    /// Unique identifier for this context.
    #[builder(default = "Uuid::now_v7()")]
    pub id: Uuid,
    /// Human-readable label for this context.
    pub name: String,
    /// Context version.
    #[schemars(with = "String")]
    pub version: Version,
    /// Optional longer description.
    #[builder(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// Reference-data entries.
    #[builder(default)]
    pub entries: Vec<ContextEntry>,
}

impl Context {
    /// Start building a new context.
    pub fn builder() -> ContextBuilder {
        ContextBuilder::default()
    }
}
