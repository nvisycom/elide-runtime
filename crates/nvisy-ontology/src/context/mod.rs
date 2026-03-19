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

use std::collections::HashMap;

use nvisy_core::content::ContentSource;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

pub use self::entry::{ContextEntry, ContextEntryData};

/// A collection of [`Context`]s keyed by their source UUID.
#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
pub struct Contexts {
    /// The contexts, keyed by source UUID.
    #[serde(flatten)]
    contexts: HashMap<Uuid, Context>,
}

impl Contexts {
    /// Create an empty collection.
    pub fn new() -> Self {
        Self::default()
    }

    /// Get a context by its source UUID.
    pub fn get(&self, id: &Uuid) -> Option<&Context> {
        self.contexts.get(id)
    }

    /// Insert a context, keyed by its source UUID. Replaces any existing
    /// context with the same ID.
    pub fn insert(&mut self, context: Context) {
        self.contexts.insert(context.source.as_uuid(), context);
    }

    /// Number of contexts in the collection.
    pub fn len(&self) -> usize {
        self.contexts.len()
    }

    /// Returns `true` if the collection is empty.
    pub fn is_empty(&self) -> bool {
        self.contexts.is_empty()
    }

    /// Returns `true` if a context with the given ID exists.
    pub fn contains(&self, id: &Uuid) -> bool {
        self.contexts.contains_key(id)
    }

    /// Iterate over all contexts.
    pub fn iter(&self) -> impl Iterator<Item = (&Uuid, &Context)> {
        self.contexts.iter()
    }

    /// Iterate over all context values.
    pub fn values(&self) -> impl Iterator<Item = &Context> {
        self.contexts.values()
    }
}

/// A persistent, reusable collection of reference data for detection.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct Context {
    /// Content source identity and lineage.
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
