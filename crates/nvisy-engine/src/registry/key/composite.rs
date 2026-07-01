//! 32-byte `(actor_id, resource_id)` key.

use derive_more::{AsRef, Deref, From};
use uuid::Uuid;

/// 32-byte actor-scoped key: `[actor_id: 16][resource_id: 16]`.
///
/// Identical to the salvaged shape; one entry per resource.
#[derive(Clone, Copy, Deref, AsRef, From)]
#[as_ref(forward)]
pub(crate) struct CompositeKey(#[deref] [u8; 32]);

impl CompositeKey {
    /// Build from an actor + resource UUID.
    pub fn new(actor_id: Uuid, resource_id: Uuid) -> Self {
        let mut key = [0u8; 32];
        key[..16].copy_from_slice(actor_id.as_bytes());
        key[16..].copy_from_slice(resource_id.as_bytes());
        Self(key)
    }

    /// Prefix bytes for "every resource of `actor`": 16 bytes.
    /// Use with `Keyspace::prefix` to range over every entry an
    /// actor owns in a 2-component keyspace.
    pub fn actor_prefix(actor_id: Uuid) -> [u8; 16] {
        *actor_id.as_bytes()
    }
}
