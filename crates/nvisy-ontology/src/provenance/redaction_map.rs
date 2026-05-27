//! Redaction map: entity-to-location index for the redaction phase.
//!
//! The [`RedactionMap<M>`] records which entities the pipeline touched
//! and where they were located. Original and replacement *values*
//! live on the corresponding [`AuditEntry::value`] (see
//! [`RedactionValue`]) — the map is a thin index, not a value store.
//!
//! [`AuditEntry::value`]: super::AuditEntry::value
//! [`RedactionValue`]: super::RedactionValue

use derive_more::{Deref, DerefMut};
use schemars::JsonSchema;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::modality::Modality;

/// One entry in the redaction map: the entity touched and where it
/// was located in the document.
///
/// Values (original / replacement) are not stored here; consult the
/// corresponding [`AuditEntry`] by `entity_id`.
///
/// [`AuditEntry`]: super::AuditEntry
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(
    rename_all = "camelCase",
    bound(serialize = "M: Serialize", deserialize = "M: DeserializeOwned",)
)]
#[schemars(bound = "M: JsonSchema")]
pub struct RedactionMapping<M: Modality> {
    /// The entity this mapping belongs to.
    pub entity_id: Uuid,
    /// Where in the document the entity was found.
    pub location: M,
}

/// Per-entity redaction lineage index.
///
/// Created during the redaction phase by the redaction evaluator.
/// Records which entities were considered for redaction and where
/// they lived in the document. Sensitive values are not duplicated
/// here — they live on the matching [`AuditEntry`].
///
/// [`AuditEntry`]: super::AuditEntry
#[derive(Debug, Clone, Deref, DerefMut)]
#[derive(Serialize, Deserialize, JsonSchema)]
#[serde(
    rename_all = "camelCase",
    bound(serialize = "M: Serialize", deserialize = "M: DeserializeOwned",)
)]
#[schemars(bound = "M: JsonSchema")]
pub struct RedactionMap<M: Modality> {
    /// Per-entity redaction mappings.
    #[deref]
    #[deref_mut]
    pub entries: Vec<RedactionMapping<M>>,
}

impl<M: Modality> Default for RedactionMap<M> {
    fn default() -> Self {
        Self {
            entries: Vec::new(),
        }
    }
}

impl<M: Modality> RedactionMap<M> {
    /// Create an empty redaction map.
    pub fn new() -> Self {
        Self::default()
    }
}
