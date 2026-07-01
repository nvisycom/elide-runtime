//! 48-byte `(actor_id, run_id, doc_id)` key.

use derive_more::{AsRef, Deref, From};
use uuid::Uuid;

/// 48-byte per-document run-body key:
/// `[actor_id: 16][run_id: 16][doc_id: 16]`.
#[derive(Clone, Copy, Deref, AsRef, From)]
#[as_ref(forward)]
pub(crate) struct RunDocKey(#[deref] [u8; 48]);

impl RunDocKey {
    /// Build from `(actor, run, doc)`.
    pub fn new(actor_id: Uuid, run_id: Uuid, doc_id: Uuid) -> Self {
        let mut key = [0u8; 48];
        key[..16].copy_from_slice(actor_id.as_bytes());
        key[16..32].copy_from_slice(run_id.as_bytes());
        key[32..].copy_from_slice(doc_id.as_bytes());
        Self(key)
    }

    /// Prefix bytes for "every doc of `(actor, run)`": 32 bytes.
    /// Use with `Keyspace::prefix` to range over every per-doc
    /// entry of one run (e.g. cascade-delete on run delete).
    pub fn run_prefix(actor_id: Uuid, run_id: Uuid) -> [u8; 32] {
        let mut prefix = [0u8; 32];
        prefix[..16].copy_from_slice(actor_id.as_bytes());
        prefix[16..].copy_from_slice(run_id.as_bytes());
        prefix
    }
}
