//! [`Deanonymizer<M>`]: inverse of [`Anonymizer<M>`].
//!
//! Recovers the original payload that was captured when an
//! [`Entity<M>`] was anonymized. The trait accepts both the entity
//! and the replacement the anonymizer wrote, so a single
//! deanonymizer registry can dispatch over implementations that
//! use either (or both) as their recovery key:
//!
//! - **Audit-keyed**: the original was stashed at apply-time in a
//!   side store keyed on `entity.id`. The impl ignores
//!   `replacement` and looks up by entity.
//! - **Self-contained**: the recovery material lives inside the
//!   replacement itself (e.g. an AES-256-GCM ciphertext blob).
//!   The impl ignores `entity` and decodes `replacement`.
//! - **Hybrid**: future operators may need both — e.g. an
//!   encrypted blob whose key is derived from the entity id.
//!
//! Operators whose redaction is mathematically one-way (`Hash`,
//! `Redact`, `Replace` without audit, `Mask` without prefix-keep,
//! `Fake` without audit) cannot implement this trait. That's a
//! feature, not a gap — calling `revert` on an irreversible
//! operator should be a type error, not a runtime `None`.
//!
//! [`Anonymizer<M>`]: super::Anonymizer
//! [`Entity<M>`]: crate::entity::Entity

use crate::Result;
use crate::entity::Entity;
use crate::modality::Modality;

/// Inverse of [`Anonymizer<M>`]. Given the entity that was
/// anonymized and the replacement the operator wrote, recover the
/// original payload.
///
/// Returns `Ok(None)` when there's nothing to recover — the
/// replacement is a "removed" / "column dropped" variant, or the
/// side store has no record for the entity. Reserve `Err` for
/// backend failures (decryption failed, storage unreachable, …).
///
/// [`Anonymizer<M>`]: super::Anonymizer
#[async_trait::async_trait]
pub trait Deanonymizer<M: Modality>: Send + Sync {
    /// Recover the original payload from `(entity, replacement)`.
    /// Audit-keyed impls read `entity.id`; self-contained impls
    /// decode `replacement`. See the module docs for which mode
    /// applies to a given operator.
    async fn revert(
        &self,
        entity: &Entity<M>,
        replacement: &M::Replacement,
    ) -> Result<Option<M::Data>>;
}
