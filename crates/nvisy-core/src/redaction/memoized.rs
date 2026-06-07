//! [`Memoized`]: caches an inner [`Anonymizer`]'s output so the
//! same input payload always yields the same replacement.
//!
//! The cache is a [`Store<M::Replacement>`] keyed by a stable hash
//! of the source payload. First call for a given payload runs
//! `inner.apply`; subsequent calls short-circuit to the cached
//! replacement. This buys document-level coherence — every mention
//! of the same payload gets the same fake — without requiring the
//! recogniser to tag coreference.
//!
//! The wrapped operator's [`LeakProfile`] is preserved as-is. There
//! is no [`Deanonymizer`] impl: the cached entry is the *fake*, not
//! the original.
//!
//! `Memoized` is wired for every modality. Whether memoization is
//! worth its cost depends on the inner operator — for cheap
//! descriptors like [`ImageReplacement::Blur`] the savings are
//! negligible; for ML-based synthesis (deepfake faces, voice
//! cloning) they're significant.
//!
//! [`Anonymizer`]: super::Anonymizer
//! [`Deanonymizer`]: super::Deanonymizer
//! [`ImageReplacement::Blur`]: super::ImageReplacement::Blur
//! [`Store<M::Replacement>`]: super::Store

use std::hash::{DefaultHasher, Hash, Hasher};
use std::sync::Arc;

use super::store::Store;
use super::{Anonymizer, LeakProfile};
use crate::Result;
use crate::entity::Entity;
use crate::modality::Modality;

/// Memoizing wrapper around any [`Anonymizer<M>`].
#[derive(Debug, Clone)]
pub struct Memoized<A, S> {
    inner: A,
    store: Arc<S>,
}

impl<A, S> Memoized<A, S> {
    /// Build a `Memoized` wrapper around `inner`, caching into
    /// `store`.
    pub fn new(inner: A, store: Arc<S>) -> Self {
        Self { inner, store }
    }

    /// Borrow the inner anonymizer.
    pub fn inner(&self) -> &A {
        &self.inner
    }

    /// Borrow the shared cache store.
    pub fn store(&self) -> &Arc<S> {
        &self.store
    }
}

#[async_trait::async_trait]
impl<M, A, S> Anonymizer<M> for Memoized<A, S>
where
    M: Modality,
    A: Anonymizer<M>,
    S: Store<M::Replacement> + 'static,
{
    fn leak_profile(&self) -> LeakProfile {
        self.inner.leak_profile()
    }

    async fn apply(&self, entity: &Entity<M>, source: &M::Data) -> Result<M::Replacement> {
        let key = data_token(source);
        if let Some(cached) = self.store.get(&key).await? {
            return Ok(cached);
        }
        let replacement = self.inner.apply(entity, source).await?;
        self.store.put(&key, replacement.clone()).await?;
        Ok(replacement)
    }
}

fn data_token<T: Hash>(payload: &T) -> String {
    let mut hasher = DefaultHasher::new();
    payload.hash(&mut hasher);
    format!("{:016x}", hasher.finish())
}
