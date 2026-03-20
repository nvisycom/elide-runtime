//! Key provider abstraction for encryption key resolution.

use std::collections::HashMap;

use nvisy_core::Result;

/// Abstraction for resolving encryption keys by identifier.
pub trait KeyProvider: Send + Sync {
    /// Returns the raw key bytes for the given `key_id`, or an error if unknown.
    fn resolve(&self, key_id: &str) -> Result<Vec<u8>>;
}

/// In-memory key store for tests and simple deployments.
#[derive(Debug, Clone)]
pub struct StaticKeyProvider {
    keys: HashMap<String, Vec<u8>>,
}

impl StaticKeyProvider {
    /// Creates a new provider from an iterator of `(key_id, key_bytes)` pairs.
    pub fn new(keys: impl IntoIterator<Item = (String, Vec<u8>)>) -> Self {
        Self {
            keys: keys.into_iter().collect(),
        }
    }
}

impl KeyProvider for StaticKeyProvider {
    fn resolve(&self, key_id: &str) -> Result<Vec<u8>> {
        self.keys.get(key_id).cloned().ok_or_else(|| {
            nvisy_core::Error::validation(
                format!("unknown key_id: {key_id}"),
                "StaticKeyProvider::resolve",
            )
        })
    }
}
