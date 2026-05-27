//! Persistent reference-data collections for detection.
//!
//! A [`Context`] holds reusable reference data — names, faces, voices,
//! patterns, embeddings — that tells detection *what to look for*.  It is
//! separate from policy (which controls *what to do* when something is found).

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
