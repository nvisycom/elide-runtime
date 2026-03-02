//! Persistent reference-data collections for detection.
//!
//! A [`Context`] holds reusable reference data — names, faces, voices,
//! patterns, embeddings — that tells detection *what to look for*.  It is
//! separate from policy (which controls *what to do* when something is found).

mod entry;

pub use entry::{ContextEntry, ContextEntryData, ContextKind};

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use nvisy_core::path::ContentSource;

/// A persistent, reusable collection of reference data for detection.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct Context {
    /// Content source identity and lineage.
    #[serde(flatten)]
    pub source: ContentSource,
    /// Human-readable label for this context.
    pub name: String,
    /// Optional longer description.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// Reference-data entries.
    pub entries: Vec<ContextEntry>,
}

impl Context {
    /// Create a new context with the given name and entries.
    pub fn new(name: impl Into<String>, entries: Vec<ContextEntry>) -> Self {
        Self {
            source: ContentSource::new(),
            name: name.into(),
            description: None,
            entries,
        }
    }

    /// Set a description on this context.
    pub fn with_description(mut self, description: impl Into<String>) -> Self {
        self.description = Some(description.into());
        self
    }
}
