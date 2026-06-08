//! In-memory [`Store<V>`] implementation suitable for batch jobs
//! and tests.
//!
//! Backing is an `Arc<RwLock<HashMap>>` so the same store can be
//! cloned cheaply and shared across multiple anonymizers without
//! external synchronisation.

use std::collections::HashMap;
use std::sync::Arc;

use nvisy_core::Result;
use nvisy_core::redaction::Store;
use tokio::sync::RwLock;

/// In-memory implementation of [`Store<V>`].
///
/// Cloning is cheap — every clone shares the same backing map. Use
/// [`Self::len`] / [`Self::is_empty`] for inspection; iterate keys
/// via [`Self::tokens`] when you need to drain.
#[derive(Debug)]
pub struct MemoryStore<V> {
    inner: Arc<RwLock<HashMap<String, V>>>,
}

impl<V> MemoryStore<V> {
    /// Build an empty store.
    pub fn new() -> Self {
        Self::default()
    }

    /// Number of token → value mappings currently held.
    pub async fn len(&self) -> usize {
        self.inner.read().await.len()
    }

    /// `true` if no mappings have been recorded yet.
    pub async fn is_empty(&self) -> bool {
        self.inner.read().await.is_empty()
    }

    /// Snapshot the token list. Useful for tests; not a stable order.
    pub async fn tokens(&self) -> Vec<String> {
        self.inner.read().await.keys().cloned().collect()
    }
}

impl<V> Default for MemoryStore<V> {
    fn default() -> Self {
        Self {
            inner: Arc::new(RwLock::new(HashMap::new())),
        }
    }
}

impl<V> Clone for MemoryStore<V> {
    fn clone(&self) -> Self {
        Self {
            inner: Arc::clone(&self.inner),
        }
    }
}

#[async_trait::async_trait]
impl<V> Store<V> for MemoryStore<V>
where
    V: Clone + Send + Sync,
{
    async fn put(&self, token: &str, value: V) -> Result<()> {
        self.inner.write().await.insert(token.to_owned(), value);
        Ok(())
    }

    async fn get(&self, token: &str) -> Result<Option<V>> {
        Ok(self.inner.read().await.get(token).cloned())
    }
}

#[cfg(test)]
mod tests {
    use nvisy_core::modality::TextData;

    use super::*;

    fn text_of(d: Option<TextData>) -> Option<String> {
        d.map(|d| d.text.as_str().to_owned())
    }

    #[tokio::test]
    async fn put_then_get_round_trips() {
        let store: MemoryStore<TextData> = MemoryStore::new();
        store.put("tok", TextData::new("alice")).await.unwrap();
        assert_eq!(
            text_of(store.get("tok").await.unwrap()),
            Some("alice".to_owned())
        );
    }

    #[tokio::test]
    async fn unknown_token_returns_none() {
        let store: MemoryStore<TextData> = MemoryStore::new();
        assert!(store.get("missing").await.unwrap().is_none());
    }

    #[tokio::test]
    async fn put_overwrites_existing() {
        let store: MemoryStore<TextData> = MemoryStore::new();
        store.put("tok", TextData::new("alice")).await.unwrap();
        store.put("tok", TextData::new("bob")).await.unwrap();
        assert_eq!(
            text_of(store.get("tok").await.unwrap()),
            Some("bob".to_owned())
        );
    }
}
