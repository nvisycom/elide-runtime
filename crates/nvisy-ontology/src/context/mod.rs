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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn push_deduplicates() {
        let id = Uuid::now_v7();
        let mut ctx = Contexts::new();
        ctx.push(id);
        ctx.push(id);
        assert_eq!(ctx.len(), 1);
    }

    #[test]
    fn push_distinct() {
        let mut ctx = Contexts::new();
        ctx.push(Uuid::now_v7());
        ctx.push(Uuid::now_v7());
        assert_eq!(ctx.len(), 2);
    }

    #[test]
    fn contains_and_ids() {
        let id = Uuid::now_v7();
        let ctx = Contexts::from_ids(vec![id]);
        assert!(ctx.contains(&id));
        assert_eq!(ctx.ids(), &[id]);
    }

    #[test]
    fn empty() {
        let ctx = Contexts::new();
        assert!(ctx.is_empty());
        assert_eq!(ctx.len(), 0);
    }
}

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
