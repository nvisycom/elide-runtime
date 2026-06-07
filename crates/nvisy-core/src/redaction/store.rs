//! [`Store<V>`]: pluggable token vault keyed on opaque strings,
//! holding cloneable values.
//!
//! [`Memoized`] uses `Store<M::Replacement>` as its cache backing —
//! same input data hashes to the same key, so the inner anonymizer
//! runs only once per distinct payload.
//!
//! Implementations pick their own backing (in-memory map, KV store,
//! KMS-backed encrypted blob).
//!
//! [`Memoized`]: super::Memoized

use crate::Result;

/// Token vault keyed on opaque strings, parameterised on the stored
/// value type `V`.
///
/// Implementations must be safe to share across tasks and serve
/// concurrent reads/writes. Keys are opaque tokens emitted by the
/// anonymizer; values are whatever payload the operator chose to
/// persist.
#[async_trait::async_trait]
pub trait Store<V: Clone + Send + Sync>: Send + Sync {
    /// Persist `value` under `token`. Overwriting an existing token
    /// replaces the prior value.
    async fn put(&self, token: &str, value: V) -> Result<()>;

    /// Look up the value previously stored under `token`. Returns
    /// `Ok(None)` for unknown tokens; reserve `Err` for backend
    /// failures.
    async fn get(&self, token: &str) -> Result<Option<V>>;
}
