//! [`Deanonymizer<M>`]: the inverse-direction redaction operator —
//! takes a previously-emitted [`M::Replacement`] and recovers the
//! original [`M::Data`].
//!
//! Only implementable for operators whose [`LeakProfile`] is
//! [`Recoverable`] — encryption with the right key, pseudonym maps,
//! token vaults. Hash and other one-way operators don't have a
//! `Deanonymizer` impl by construction.
//!
//! [`LeakProfile`]: super::LeakProfile
//! [`M::Data`]: nvisy_core::modality::ModalityData::Data
//! [`M::Replacement`]: super::Redactable::Replacement
//! [`Recoverable`]: super::LeakProfile::Recoverable

use async_trait::async_trait;
use nvisy_core::Result;
use nvisy_core::modality::ModalityData;

use super::Redactable;

/// Inverse of [`Anonymizer<M>`] — given an emitted
/// [`M::Replacement`], recover the original payload.
///
/// [`Anonymizer<M>`]: super::Anonymizer
/// [`M::Replacement`]: super::Redactable::Replacement
#[async_trait]
pub trait Deanonymizer<M: Redactable + ModalityData>: Send + Sync {
    /// Recover the original payload from `replacement`.
    async fn revert(&self, replacement: &M::Replacement) -> Result<M::Data>;
}
