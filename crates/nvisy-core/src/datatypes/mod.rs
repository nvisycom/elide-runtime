//! Domain data types for the nvisy pipeline.
//!
//! This module defines the core data structures that flow through the nvisy
//! processing pipeline: blobs, documents, entities, redactions, audits,
//! policies, and images.

use serde::{Deserialize, Serialize};
use uuid::Uuid;

pub mod audit;
pub mod blob;
pub mod document;
pub mod entity;
pub mod image;
pub mod policy;
pub mod redaction;
pub mod redaction_context;

/// General-purpose metadata map.
pub type Metadata = serde_json::Map<String, serde_json::Value>;

/// Common fields shared by all domain data items.
///
/// Every first-class object in the pipeline (blobs, documents, entities, etc.)
/// embeds a `DataItem` to carry a unique identifier, an optional parent
/// lineage link, and arbitrary metadata.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct DataItem {
    /// Unique identifier for this item, generated as a v4 UUID on creation.
    pub id: Uuid,
    /// Identifier of the item this was derived from, if any.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parent_id: Option<Uuid>,
    /// Arbitrary key-value metadata associated with this item.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<Metadata>,
}

impl DataItem {
    /// Create a new `DataItem` with a freshly generated UUID and no parent or metadata.
    pub fn new() -> Self {
        Self {
            id: Uuid::new_v4(),
            parent_id: None,
            metadata: None,
        }
    }

    /// Attach metadata to this item (builder pattern).
    pub fn with_metadata(mut self, metadata: Metadata) -> Self {
        self.metadata = Some(metadata);
        self
    }

    /// Set `parent_id` to the id of `parent`, establishing lineage.
    pub fn derive_from(mut self, parent: &DataItem) -> Self {
        self.parent_id = Some(parent.id);
        self
    }
}

impl Default for DataItem {
    fn default() -> Self {
        Self::new()
    }
}

