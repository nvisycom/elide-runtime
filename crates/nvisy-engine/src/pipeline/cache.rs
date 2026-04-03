//! Ref-counted context cache shared across pipeline runs.
//!
//! The [`ContextCache`] lives on the [`Engine`] and deduplicates
//! context storage across concurrent runs. Each run acquires the
//! contexts it needs (loading from the registry on first access)
//! via [`ContextCache::acquire`], which returns a [`ContextGuard`]
//! that automatically releases the contexts when dropped.
//!
//! Contexts with zero references are evicted immediately on release.
//!
//! [`Engine`]: super::Engine

use std::collections::HashMap;
use std::sync::Arc;

use nvisy_ontology::context::Context;
use tokio::sync::RwLock;
use uuid::Uuid;

use crate::registry::Registry;

const TARGET: &str = "nvisy_engine::pipeline::cache";

struct CachedEntry {
    context: Arc<Context>,
    ref_count: usize,
}

/// Shared, ref-counted context cache.
///
/// Contexts are loaded from the [`Registry`] on first access and kept
/// in memory as long as at least one run holds a reference. When all
/// runs release a context (via [`ContextGuard`] drop), entries that
/// drop to zero references are evicted.
///
/// The cache is cheaply cloneable (`Arc` internally).
#[derive(Clone)]
pub struct ContextCache {
    inner: Arc<RwLock<HashMap<Uuid, CachedEntry>>>,
}

impl ContextCache {
    /// Create an empty cache.
    pub fn new() -> Self {
        Self {
            inner: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Acquire a set of context IDs for a run.
    ///
    /// Returns a [`ContextGuard`] that holds the acquired IDs and
    /// releases them (decrementing ref counts) when dropped. Contexts
    /// not yet in the cache are loaded from the registry; failures are
    /// logged and skipped.
    ///
    /// The write lock is only held for the brief insert/increment
    /// phase: registry I/O happens outside the lock.
    pub async fn acquire(
        &self,
        actor_id: Uuid,
        context_ids: &[Uuid],
        registry: &Registry,
    ) -> ContextGuard {
        // Phase 1: check which IDs are already cached (read lock).
        let missing: Vec<Uuid> = {
            let cache = self.inner.read().await;
            context_ids
                .iter()
                .filter(|id| !cache.contains_key(id))
                .copied()
                .collect()
        };

        // Phase 2: load missing contexts outside the lock.
        let mut loaded = Vec::with_capacity(missing.len());
        for id in &missing {
            if let Some(context) = load_context(actor_id, *id, registry).await {
                loaded.push((*id, Arc::new(context)));
            }
        }

        // Phase 3: insert new entries and increment ref counts (write lock).
        let mut acquired = Vec::with_capacity(context_ids.len());
        {
            let mut cache = self.inner.write().await;

            // Insert freshly loaded contexts.
            for (id, context) in loaded {
                cache.entry(id).or_insert(CachedEntry {
                    context,
                    ref_count: 0,
                });
            }

            // Increment ref counts for all requested IDs that are now cached.
            for &id in context_ids {
                if let Some(entry) = cache.get_mut(&id) {
                    entry.ref_count += 1;
                    acquired.push(id);
                }
            }
        }

        tracing::debug!(
            target: TARGET,
            requested = context_ids.len(),
            acquired = acquired.len(),
            freshly_loaded = missing.len(),
            "contexts acquired",
        );

        ContextGuard {
            cache: self.clone(),
            ids: acquired,
        }
    }

    /// Decrement ref counts and evict entries that reach zero.
    async fn release(&self, context_ids: &[Uuid]) {
        let mut cache = self.inner.write().await;
        let mut evicted = 0usize;

        for id in context_ids {
            if let Some(entry) = cache.get_mut(id) {
                entry.ref_count = entry.ref_count.saturating_sub(1);
                if entry.ref_count == 0 {
                    cache.remove(id);
                    evicted += 1;
                    tracing::trace!(target: TARGET, %id, "context evicted from cache");
                }
            }
        }

        if evicted > 0 {
            tracing::debug!(
                target: TARGET,
                evicted,
                remaining = cache.len(),
                "contexts released",
            );
        }
    }

    /// Get a context by ID, if cached.
    pub async fn get(&self, id: &Uuid) -> Option<Arc<Context>> {
        self.inner
            .read()
            .await
            .get(id)
            .map(|e| Arc::clone(&e.context))
    }

    /// Resolve a list of context IDs into their cached contexts.
    pub async fn resolve(&self, ids: &[Uuid]) -> Vec<Arc<Context>> {
        let cache = self.inner.read().await;
        ids.iter()
            .filter_map(|id| cache.get(id).map(|e| Arc::clone(&e.context)))
            .collect()
    }

    /// Number of contexts currently cached.
    pub async fn len(&self) -> usize {
        self.inner.read().await.len()
    }
}

impl Default for ContextCache {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Debug for ContextCache {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ContextCache").finish_non_exhaustive()
    }
}

/// RAII guard that releases acquired contexts when dropped.
///
/// Returned by [`ContextCache::acquire`]. Dropping the guard
/// decrements ref counts and evicts entries that reach zero.
///
/// The guard also provides [`ids`](Self::ids) to inspect which
/// context IDs were successfully acquired.
pub struct ContextGuard {
    cache: ContextCache,
    ids: Vec<Uuid>,
}

impl ContextGuard {
    /// The context IDs that were successfully acquired.
    pub fn ids(&self) -> &[Uuid] {
        &self.ids
    }
}

impl Drop for ContextGuard {
    fn drop(&mut self) {
        if self.ids.is_empty() {
            return;
        }
        let cache = self.cache.clone();
        let ids = std::mem::take(&mut self.ids);
        // Spawn a task to release asynchronously: Drop is synchronous.
        tokio::spawn(async move {
            cache.release(&ids).await;
        });
    }
}

/// Load a single context from the registry, returning `None` on failure.
async fn load_context(actor_id: Uuid, id: Uuid, registry: &Registry) -> Option<Context> {
    match registry.read_context(actor_id, id).await {
        Ok(context) => Some(context),
        Err(e) => {
            tracing::warn!(target: TARGET, %id, error = %e, "failed to load context");
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use nvisy_ontology::context::Context;

    use super::*;

    fn test_context(name: &str) -> Context {
        Context::builder()
            .with_name(name)
            .with_version(semver::Version::new(1, 0, 0))
            .build()
            .unwrap()
    }

    /// Insert a context directly into the cache for testing (bypasses registry).
    async fn insert(cache: &ContextCache, ctx: Context, ref_count: usize) -> Uuid {
        let id = ctx.id;
        let mut inner = cache.inner.write().await;
        inner.insert(
            id,
            CachedEntry {
                context: Arc::new(ctx),
                ref_count,
            },
        );
        id
    }

    #[tokio::test]
    async fn release_evicts_at_zero() {
        let cache = ContextCache::new();
        let id = insert(&cache, test_context("test"), 1).await;

        assert!(cache.get(&id).await.is_some());

        cache.release(&[id]).await;
        assert_eq!(cache.len().await, 0);
        assert!(cache.get(&id).await.is_none());
    }

    #[tokio::test]
    async fn shared_refs_prevent_eviction() {
        let cache = ContextCache::new();
        let id = insert(&cache, test_context("shared"), 2).await;

        cache.release(&[id]).await;
        assert_eq!(cache.len().await, 1);

        cache.release(&[id]).await;
        assert_eq!(cache.len().await, 0);
    }

    #[tokio::test]
    async fn resolve_returns_matching_contexts() {
        let cache = ContextCache::new();
        let id1 = insert(&cache, test_context("alpha"), 1).await;
        let id2 = insert(&cache, test_context("beta"), 1).await;
        let missing = Uuid::new_v4();

        let resolved = cache.resolve(&[id1, missing, id2]).await;
        assert_eq!(resolved.len(), 2);
        assert_eq!(resolved[0].name, "alpha");
        assert_eq!(resolved[1].name, "beta");
    }

    #[tokio::test]
    async fn get_returns_arc() {
        let cache = ContextCache::new();
        let id = insert(&cache, test_context("shared-ref"), 1).await;

        let a = cache.get(&id).await.unwrap();
        let b = cache.get(&id).await.unwrap();
        assert!(Arc::ptr_eq(&a, &b));
    }

    #[tokio::test]
    async fn guard_releases_on_drop() {
        let cache = ContextCache::new();
        let id = insert(&cache, test_context("guarded"), 0).await;

        // Simulate what acquire does: bump ref_count.
        {
            let mut inner = cache.inner.write().await;
            inner.get_mut(&id).unwrap().ref_count = 1;
        }

        let guard = ContextGuard {
            cache: cache.clone(),
            ids: vec![id],
        };
        drop(guard);

        // Give the spawned release task time to run.
        tokio::task::yield_now().await;
        tokio::task::yield_now().await;

        assert_eq!(cache.len().await, 0);
    }
}
