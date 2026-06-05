//! [`EntityRecord`]: a detected entity bundled with the optional
//! audit entry produced for it during redaction.

use nvisy_core::entity::Entity;
use schemars::JsonSchema;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};

use super::entry::AuditEntry;
use crate::modality::DocumentModality;

/// One per-entity record on an [`Audit<M>`]: the detected entity
/// plus the audit entry produced for it during redaction (if any).
///
/// `audit` is `None` when the entity was detected but no
/// redaction or suppression rule matched — the detection still
/// shows up in the compliance trail but no redaction decision
/// was made. Suppressed-and-recorded ends up as `Some(entry)`
/// with `entry.execution == Execution::Suppressed`.
///
/// Bundling the two together replaces the previous
/// `Vec<Entity> + Vec<AuditEntry>` pair indexed by `entity_id`:
/// every consumer that needed both (dedup, applicator, validator,
/// exporter) used to join the two vectors via a hash map. The
/// bundled shape removes the join, the duplicated identifier, and
/// the entire `RedactionMap` denormalization that used to mirror
/// `entity.location`.
///
/// [`Audit<M>`]: super::Audit
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(
    rename_all = "camelCase",
    bound(
        serialize = "M: Serialize, M::Replacement: Serialize",
        deserialize = "M: DeserializeOwned, M::Replacement: DeserializeOwned",
    )
)]
#[schemars(bound = "M: JsonSchema, M::Replacement: JsonSchema")]
pub struct EntityRecord<M: DocumentModality> {
    /// The detected entity.
    pub entity: Entity<M>,
    /// The redaction record for this entity, if a strategy decided
    /// to act on it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub audit: Option<AuditEntry<M>>,
}

impl<M: DocumentModality> EntityRecord<M> {
    /// Wrap an [`Entity<M>`] with no audit decision yet.
    pub fn new(entity: Entity<M>) -> Self {
        Self {
            entity,
            audit: None,
        }
    }
}
