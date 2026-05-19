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
use derive_more::{Deref, DerefMut, From};
use schemars::JsonSchema;
use semver::Version;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

pub use self::entry::{ContextEntry, ContextEntryData};

/// Lightweight set of context references carried by each document envelope.
///
/// Each UUID points to a [`Context`] in the engine's context cache.
#[derive(Debug, Clone, Default, Deref, DerefMut, From)]
#[derive(Serialize, Deserialize, JsonSchema)]
pub struct Contexts(Vec<Uuid>);

impl Contexts {
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a context ID, deduplicating.
    pub fn push(&mut self, id: Uuid) {
        if !self.0.contains(&id) {
            self.0.push(id);
        }
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
