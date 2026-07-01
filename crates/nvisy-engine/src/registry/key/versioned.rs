//! Variable-length `(actor, resource, version)` key with
//! big-endian semver ordering.

use semver::Version;
use uuid::Uuid;

/// Variable-length per-version key:
/// `[actor: 16][resource: 16][version_be_serialised: N]`.
///
/// The trailing version bytes use a stable big-endian encoding of
/// `(major, minor, patch)` u64s — `[major: 8][minor: 8][patch: 8]`
/// — so lex order on the byte sequence matches semver order. Pre-
/// release / build metadata are dropped from the key (they ride
/// along in the value); two `(id, version)` pairs are considered
/// distinct if and only if their `(major, minor, patch)` differs.
pub(crate) struct VersionedKey(Vec<u8>);

impl VersionedKey {
    /// Build from `(actor, id, version)`.
    pub fn new(actor_id: Uuid, resource_id: Uuid, version: &Version) -> Self {
        let mut buf = Vec::with_capacity(16 + 16 + 24);
        buf.extend_from_slice(actor_id.as_bytes());
        buf.extend_from_slice(resource_id.as_bytes());
        buf.extend_from_slice(&version.major.to_be_bytes());
        buf.extend_from_slice(&version.minor.to_be_bytes());
        buf.extend_from_slice(&version.patch.to_be_bytes());
        Self(buf)
    }

    /// Borrow the underlying bytes for fjall.
    pub fn as_bytes(&self) -> &[u8] {
        &self.0
    }

    /// Prefix bytes for "every version of `(actor, id)`": 32 bytes,
    /// just `[actor][id]`. Use with `Keyspace::prefix` to range over
    /// all versions of one logical resource.
    pub fn prefix(actor_id: Uuid, resource_id: Uuid) -> [u8; 32] {
        let mut prefix = [0u8; 32];
        prefix[..16].copy_from_slice(actor_id.as_bytes());
        prefix[16..].copy_from_slice(resource_id.as_bytes());
        prefix
    }

    /// Prefix bytes for "every resource of `actor`": 16 bytes.
    pub fn actor_prefix(actor_id: Uuid) -> [u8; 16] {
        *actor_id.as_bytes()
    }

    /// Read the version triple back from a full versioned-key byte
    /// slice (last 24 bytes).
    pub fn version_from_bytes(bytes: &[u8]) -> Option<(u64, u64, u64)> {
        if bytes.len() < 32 + 24 {
            return None;
        }
        let tail = &bytes[32..32 + 24];
        let major = u64::from_be_bytes(tail[..8].try_into().ok()?);
        let minor = u64::from_be_bytes(tail[8..16].try_into().ok()?);
        let patch = u64::from_be_bytes(tail[16..24].try_into().ok()?);
        Some((major, minor, patch))
    }

    /// Read the resource id (bytes 16..32) from a full versioned-key
    /// byte slice.
    pub fn resource_id_from_bytes(bytes: &[u8]) -> Option<Uuid> {
        if bytes.len() < 32 {
            return None;
        }
        let mut b = [0u8; 16];
        b.copy_from_slice(&bytes[16..32]);
        Some(Uuid::from_bytes(b))
    }
}
