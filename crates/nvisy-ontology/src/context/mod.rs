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

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

pub use self::entry::{ContextEntry, ContextEntryData};
use crate::entity::ContentSource;

/// Lightweight set of context references carried by each document envelope.
///
/// Each UUID points to a [`Context`] in the engine's context cache.
#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
pub struct Contexts(Vec<Uuid>);

impl Contexts {
    pub fn new() -> Self {
        Self(Vec::new())
    }

    pub fn from_ids(ids: Vec<Uuid>) -> Self {
        Self(ids)
    }

    pub fn push(&mut self, id: Uuid) {
        if !self.0.contains(&id) {
            self.0.push(id);
        }
    }

    pub fn ids(&self) -> &[Uuid] {
        &self.0
    }

    pub fn len(&self) -> usize {
        self.0.len()
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    pub fn contains(&self, id: &Uuid) -> bool {
        self.0.contains(id)
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
