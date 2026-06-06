//! Key provider abstraction for encryption key resolution.

use std::collections::HashMap;
use std::fmt;
use std::sync::Arc;

use base64::Engine;
use bytes::Bytes;
use nvisy_core::{Error, Result};
/// Abstraction for resolving encryption keys by identifier.
pub trait KeyProvider: Send + Sync {
    /// Returns the raw key bytes for the given `key_id`, or an error if unknown.
    fn resolve(&self, key_id: &str) -> Result<Bytes>;
}

/// Shared, cheaply-clonable handle to a [`KeyProvider`].
#[derive(Clone)]
pub struct SharedKeyProvider(Arc<dyn KeyProvider>);

impl SharedKeyProvider {
    /// Wrap a concrete [`KeyProvider`] implementation.
    pub fn new(provider: impl KeyProvider + 'static) -> Self {
        Self(Arc::new(provider))
    }
}

impl KeyProvider for SharedKeyProvider {
    fn resolve(&self, key_id: &str) -> Result<Bytes> {
        self.0.resolve(key_id)
    }
}

impl Default for SharedKeyProvider {
    fn default() -> Self {
        Self::new(StaticKeyProvider::default())
    }
}

impl fmt::Debug for SharedKeyProvider {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("SharedKeyProvider").finish()
    }
}

/// In-memory key store for tests and simple deployments.
#[derive(Debug, Clone, Default)]
pub struct StaticKeyProvider {
    keys: HashMap<String, Bytes>,
}

impl StaticKeyProvider {
    /// Creates a new provider from an iterator of `(key_id, key_bytes)` pairs.
    pub fn new(keys: impl IntoIterator<Item = (String, Vec<u8>)>) -> Self {
        Self {
            keys: keys
                .into_iter()
                .map(|(id, bytes)| (id, Bytes::from(bytes)))
                .collect(),
        }
    }

    /// Insert a single key, replacing any existing entry with the same ID.
    pub fn insert(&mut self, key_id: impl Into<String>, key: impl Into<Bytes>) {
        self.keys.insert(key_id.into(), key.into());
    }

    /// Build from a JSON value.
    ///
    /// Expects an object whose keys are key IDs and values are
    /// base64-encoded key bytes. Any other shape returns an empty provider.
    pub fn from_json(value: &serde_json::Value) -> Result<Self> {
        let Some(map) = value.as_object() else {
            return Ok(Self::default());
        };

        let mut keys = HashMap::with_capacity(map.len());
        for (id, val) in map {
            let encoded = val.as_str().ok_or_else(|| {
                Error::validation(
                    format!("key \"{id}\" must be a base64-encoded string"),
                    "StaticKeyProvider::from_json",
                )
            })?;
            let bytes = base64::engine::general_purpose::STANDARD
                .decode(encoded)
                .map_err(|e| {
                    Error::validation(
                        format!("key \"{id}\" is not valid base64: {e}"),
                        "StaticKeyProvider::from_json",
                    )
                })?;
            keys.insert(id.clone(), Bytes::from(bytes));
        }

        Ok(Self { keys })
    }
}

impl KeyProvider for StaticKeyProvider {
    fn resolve(&self, key_id: &str) -> Result<Bytes> {
        self.keys.get(key_id).cloned().ok_or_else(|| {
            Error::validation(
                format!("unknown key_id: {key_id}"),
                "StaticKeyProvider::resolve",
            )
        })
    }
}
