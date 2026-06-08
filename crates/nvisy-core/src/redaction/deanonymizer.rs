//! [`Deanonymizer<M>`]: audit-keyed inverse of [`Anonymizer<M>`].
//!
//! Recovers the original payload that was captured when an
//! [`Entity<M>`] was anonymized. Implemented by wrappers that
//! persist the original at apply-time keyed on `entity.id`. Works
//! regardless of what visible value the operator emitted, because
//! the recovery key isn't in the output — it's the audit record.
//!
//! Operators whose output already contains the recovery material
//! (e.g. `Decrypt` reading a ciphertext blob) expose an inherent
//! `decode` method instead — they don't fit the audit-keyed shape
//! and don't need to.
//!
//! [`Anonymizer<M>`]: super::Anonymizer
//! [`Entity<M>`]: crate::entity::Entity

use crate::Result;
use crate::entity::Entity;
use crate::modality::Modality;

/// Audit-keyed inverse of [`Anonymizer<M>`]. Given the entity that
/// was anonymized, look up the stored original.
///
/// Returns `Ok(None)` when no original was stored for the entity
/// (the operator didn't persist anything, or the store has been
/// reset). Reserve `Err` for backend failures.
///
/// [`Anonymizer<M>`]: super::Anonymizer
#[async_trait::async_trait]
pub trait Deanonymizer<M: Modality>: Send + Sync {
    /// Recover the original payload that was captured when `entity`
    /// was anonymized, or `None` when nothing was stored for it.
    async fn revert(&self, entity: &Entity<M>) -> Result<Option<M::Data>>;
}
