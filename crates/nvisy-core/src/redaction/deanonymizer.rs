//! [`Deanonymizer<M>`]: the inverse-direction redaction operator —
//! takes a previously-emitted [`Modality::Replacement`] and recovers
//! the original [`Modality::Data`].
//!
//! Only implementable for operators whose [`LeakProfile`] is
//! [`LeakProfile::Recoverable`] — encryption with the right key,
//! pseudonym maps, token vaults. Hash and other one-way operators
//! don't have a `Deanonymizer` impl by construction.
//!
//! [`LeakProfile`]: super::LeakProfile

use async_trait::async_trait;

use crate::Result;
use crate::modality::Modality;

/// Inverse of [`Anonymizer<M>`] — given an emitted
/// [`Modality::Replacement`], recover the original payload.
///
/// [`Anonymizer<M>`]: super::Anonymizer
#[async_trait]
pub trait Deanonymizer<M: Modality>: Send + Sync {
    /// Recover the original payload from `replacement`.
    async fn revert(&self, replacement: &M::Replacement) -> Result<M::Data>;
}
