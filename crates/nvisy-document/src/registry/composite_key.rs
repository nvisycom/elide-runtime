//! Actor-scoped composite key for registry storage.

use derive_more::{AsRef, Deref, From};
use uuid::Uuid;

/// 32-byte actor-scoped key: `[actor_id: 16][resource_id: 16]`.
///
/// Every registry entry is scoped to an actor. This type encodes
/// that invariant and provides ergonomic construction from two UUIDs.
#[derive(Clone, Copy, Deref, AsRef, From)]
#[as_ref(forward)]
pub(crate) struct CompositeKey(#[deref] [u8; 32]);

impl CompositeKey {
    /// Build a key from an actor UUID and a resource UUID.
    pub fn new(actor_id: Uuid, resource_id: Uuid) -> Self {
        let mut key = [0u8; 32];
        key[..16].copy_from_slice(actor_id.as_bytes());
        key[16..].copy_from_slice(resource_id.as_bytes());
        Self(key)
    }

    /// Extract the actor UUID from the leading 16 bytes.
    pub fn actor_id(&self) -> Uuid {
        let mut bytes = [0u8; 16];
        bytes.copy_from_slice(&self.0[..16]);
        Uuid::from_bytes(bytes)
    }

    /// Extract the resource UUID from the trailing 16 bytes.
    pub fn resource_id(&self) -> Uuid {
        let mut bytes = [0u8; 16];
        bytes.copy_from_slice(&self.0[16..]);
        Uuid::from_bytes(bytes)
    }
}
