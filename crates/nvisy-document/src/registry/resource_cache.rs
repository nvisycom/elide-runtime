//! Generic ref-counted resource cache shared across pipeline runs.
//!
//! [`ResourceCache<T>`] deduplicates in-memory resources across
//! concurrent runs. Each run acquires the resources it needs via
//! [`acquire`], which loads missing entries from a caller-supplied
//! async loader. The returned [`ResourceGuard`] automatically releases
//! resources (decrementing ref counts) when dropped; entries that reach
//! zero references are evicted immediately.
//!
//! Used by the [`Registry`] to provide context and policy caches.
//!
//! [`acquire`]: ResourceCache::acquire
//! [`Registry`]: super::Registry

use std::collections::HashMap;
use std::future::Future;
use std::sync::Arc;
use std::{fmt, mem};

use tokio::sync::RwLock;
use uuid::Uuid;

const TARGET: &str = "nvisy_document::registry::cache";

/// A cached entry with its value and reference count.
struct CachedEntry<T> {
    value: Arc<T>,
    ref_count: usize,
}

/// Generic ref-counted resource cache.
///
/// Resources are loaded on first access via a caller-supplied async
/// loader and kept in memory as long as at least one run holds a
/// reference. When all runs release a resource (via [`ResourceGuard`]
/// drop), entries that reach zero references are evicted.
///
/// Cheaply cloneable (`Arc` internally).
///
/// # Type parameters
///
/// - `T`: the cached resource type (e.g. [`Policy`]).
///
/// [`Policy`]: crate::policy::Policy
pub struct ResourceCache<T> {
    label: &'static str,
    inner: Arc<RwLock<HashMap<Uuid, CachedEntry<T>>>>,
}

impl<T: Send + Sync + 'static> ResourceCache<T> {
    /// Create an empty cache with the given diagnostic label.
    ///
    /// The label appears in tracing spans (e.g. `"context"`, `"policy"`).
    pub fn new(label: &'static str) -> Self {
        Self {
            label,
            inner: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Acquire a set of resource IDs for a run.
    ///
    /// Returns a [`ResourceGuard`] that holds the acquired IDs and
    /// releases them (decrementing ref counts) when dropped. Resources
    /// not yet in the cache are loaded via `load_fn`; failures are
    /// logged and skipped.
    ///
    /// The write lock is only held for the brief insert/increment
    /// phase — the loader runs outside the lock.
    pub async fn acquire<F, Fut>(&self, ids: &[Uuid], load_fn: F) -> ResourceGuard<T>
    where
        F: Fn(Uuid) -> Fut,
        Fut: Future<Output = Option<T>>,
    {
        // Phase 1: check which IDs are already cached (read lock).
        let missing: Vec<Uuid> = {
            let cache = self.inner.read().await;
            ids.iter()
                .filter(|id| !cache.contains_key(id))
                .copied()
                .collect()
        };

        // Phase 2: load missing resources outside the lock.
        let mut loaded = Vec::with_capacity(missing.len());
        for id in &missing {
            if let Some(value) = load_fn(*id).await {
                loaded.push((*id, Arc::new(value)));
            }
        }

        // Phase 3: insert new entries and increment ref counts (write lock).
        let mut acquired = Vec::with_capacity(ids.len());
        {
            let mut cache = self.inner.write().await;

            for (id, value) in loaded {
                cache.entry(id).or_insert(CachedEntry {
                    value,
                    ref_count: 0,
                });
            }

            for &id in ids {
                if let Some(entry) = cache.get_mut(&id) {
                    entry.ref_count += 1;
                    acquired.push(id);
                }
            }
        }

        tracing::debug!(
            target: TARGET,
            kind = self.label,
            requested = ids.len(),
            acquired = acquired.len(),
            freshly_loaded = missing.len(),
            "resources acquired",
        );

        ResourceGuard {
            cache: self.clone(),
            ids: acquired,
        }
    }

    /// Decrement ref counts and evict entries that reach zero.
    async fn release(&self, ids: &[Uuid]) {
        let mut cache = self.inner.write().await;
        let mut evicted = 0usize;

        for id in ids {
            if let Some(entry) = cache.get_mut(id) {
                entry.ref_count = entry.ref_count.saturating_sub(1);
                if entry.ref_count == 0 {
                    cache.remove(id);
                    evicted += 1;
                    tracing::trace!(target: TARGET, %id, kind = self.label, "evicted");
                }
            }
        }

        if evicted > 0 {
            tracing::debug!(
                target: TARGET,
                kind = self.label,
                evicted,
                remaining = cache.len(),
                "resources released",
            );
        }
    }

    /// Get a resource by ID, if cached.
    pub async fn get(&self, id: &Uuid) -> Option<Arc<T>> {
        self.inner
            .read()
            .await
            .get(id)
            .map(|e| Arc::clone(&e.value))
    }

    /// Resolve a list of IDs into their cached values.
    ///
    /// IDs not present in the cache are silently skipped.
    pub async fn resolve(&self, ids: &[Uuid]) -> Vec<Arc<T>> {
        let cache = self.inner.read().await;
        ids.iter()
            .filter_map(|id| cache.get(id).map(|e| Arc::clone(&e.value)))
            .collect()
    }

    /// Number of resources currently cached.
    pub async fn len(&self) -> usize {
        self.inner.read().await.len()
    }

    /// Whether the cache is empty.
    pub async fn is_empty(&self) -> bool {
        self.inner.read().await.is_empty()
    }
}

impl<T> Clone for ResourceCache<T> {
    fn clone(&self) -> Self {
        Self {
            label: self.label,
            inner: Arc::clone(&self.inner),
        }
    }
}

impl<T> fmt::Debug for ResourceCache<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ResourceCache")
            .field("label", &self.label)
            .finish_non_exhaustive()
    }
}

/// RAII guard that releases acquired resources when dropped.
///
/// Returned by [`ResourceCache::acquire`]. Dropping the guard
/// decrements ref counts and evicts entries that reach zero.
///
/// # Runtime lifetime requirement
///
/// `Drop` spawns a tokio task to call the cache's `release`
/// asynchronously rather than blocking the dropping thread. This
/// works only as long as the tokio runtime outlives every
/// `ResourceGuard` instance — if the runtime shuts down before a
/// spawned release runs, ref counts leak and the corresponding
/// entries stay cached for the remaining process lifetime.
///
/// In practice the engine holds the runtime for the duration of a
/// run, and `ResourceGuard` instances live no longer than the run
/// that acquired them, so the assumption holds. Test code that
/// constructs guards under a short-lived runtime should `drop` the
/// guard explicitly before tearing the runtime down.
pub struct ResourceGuard<T: Send + Sync + 'static> {
    cache: ResourceCache<T>,
    ids: Vec<Uuid>,
}

impl<T: Send + Sync + 'static> ResourceGuard<T> {
    /// The resource IDs that were successfully acquired.
    pub fn ids(&self) -> &[Uuid] {
        &self.ids
    }
}

impl<T: Send + Sync + 'static> Drop for ResourceGuard<T> {
    fn drop(&mut self) {
        if self.ids.is_empty() {
            return;
        }
        let cache = self.cache.clone();
        let ids = mem::take(&mut self.ids);
        // Spawned, not awaited, so Drop stays non-blocking — see
        // the runtime-lifetime note on the struct doc.
        tokio::spawn(async move {
            cache.release(&ids).await;
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn release_evicts_at_zero() {
        let cache = ResourceCache::<String>::new("test");
        insert(&cache, Uuid::nil(), "hello".to_string(), 1).await;

        assert!(cache.get(&Uuid::nil()).await.is_some());

        cache.release(&[Uuid::nil()]).await;
        assert_eq!(cache.len().await, 0);
        assert!(cache.get(&Uuid::nil()).await.is_none());
    }

    #[tokio::test]
    async fn shared_refs_prevent_eviction() {
        let cache = ResourceCache::<String>::new("test");
        let id = Uuid::nil();
        insert(&cache, id, "shared".to_string(), 2).await;

        cache.release(&[id]).await;
        assert_eq!(cache.len().await, 1);

        cache.release(&[id]).await;
        assert_eq!(cache.len().await, 0);
    }

    #[tokio::test]
    async fn resolve_returns_matching() {
        let cache = ResourceCache::<String>::new("test");
        let id1 = Uuid::from_u128(1);
        let id2 = Uuid::from_u128(2);
        let missing = Uuid::from_u128(99);
        insert(&cache, id1, "alpha".to_string(), 1).await;
        insert(&cache, id2, "beta".to_string(), 1).await;

        let resolved = cache.resolve(&[id1, missing, id2]).await;
        assert_eq!(resolved.len(), 2);
        assert_eq!(*resolved[0], "alpha");
        assert_eq!(*resolved[1], "beta");
    }

    #[tokio::test]
    async fn get_returns_arc() {
        let cache = ResourceCache::<String>::new("test");
        let id = Uuid::nil();
        insert(&cache, id, "shared-ref".to_string(), 1).await;

        let a = cache.get(&id).await.unwrap();
        let b = cache.get(&id).await.unwrap();
        assert!(Arc::ptr_eq(&a, &b));
    }

    #[tokio::test]
    async fn acquire_loads_and_caches() {
        let cache = ResourceCache::<String>::new("test");
        let id = Uuid::from_u128(42);

        let guard = cache
            .acquire(&[id], |_| async { Some("loaded".to_string()) })
            .await;

        assert_eq!(guard.ids(), &[id]);
        assert_eq!(*cache.get(&id).await.unwrap(), "loaded");
    }

    #[tokio::test]
    async fn acquire_skips_failed_loads() {
        let cache = ResourceCache::<String>::new("test");
        let id = Uuid::from_u128(1);

        let guard = cache.acquire(&[id], |_| async { None }).await;

        assert!(guard.ids().is_empty());
        assert!(cache.is_empty().await);
    }

    #[tokio::test]
    async fn guard_releases_on_drop() {
        let cache = ResourceCache::<String>::new("test");
        let id = Uuid::nil();
        insert(&cache, id, "guarded".to_string(), 0).await;

        // Simulate what acquire does: bump ref_count.
        {
            let mut inner = cache.inner.write().await;
            inner.get_mut(&id).unwrap().ref_count = 1;
        }

        let guard = ResourceGuard {
            cache: cache.clone(),
            ids: vec![id],
        };
        drop(guard);

        // Give the spawned release task time to run.
        tokio::task::yield_now().await;
        tokio::task::yield_now().await;

        assert_eq!(cache.len().await, 0);
    }

    async fn insert<T: Send + Sync + 'static>(
        cache: &ResourceCache<T>,
        id: Uuid,
        value: T,
        ref_count: usize,
    ) {
        let mut inner = cache.inner.write().await;
        inner.insert(
            id,
            CachedEntry {
                value: Arc::new(value),
                ref_count,
            },
        );
    }
}
