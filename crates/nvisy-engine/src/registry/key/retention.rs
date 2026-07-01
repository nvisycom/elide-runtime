//! 33-byte `(actor, file, scope_byte)` key.
//!
//! The `scope_byte` is a stable u8 map over [`RetentionScope`]
//! — the on-disk bytes are part of the format, so the mapping
//! must not be renumbered. Prefix scan by `(actor, file)`
//! returns every retention row for one file.

use derive_more::{AsRef, Deref, From};
use nvisy_core::policy::RetentionScope;
use uuid::Uuid;

/// 33-byte retention key:
/// `[actor: 16][file: 16][scope: 1]`.
#[derive(Clone, Copy, Deref, AsRef, From)]
#[as_ref(forward)]
pub(crate) struct RetentionKey(#[deref] [u8; 33]);

impl RetentionKey {
    /// Build from `(actor, file, scope)`.
    pub fn new(actor_id: Uuid, file_id: Uuid, scope: RetentionScope) -> Self {
        let mut key = [0u8; 33];
        key[..16].copy_from_slice(actor_id.as_bytes());
        key[16..32].copy_from_slice(file_id.as_bytes());
        key[32] = scope_to_byte(scope);
        Self(key)
    }

    /// Prefix bytes for "every retention row for `(actor, file)`":
    /// 32 bytes. Use with `Keyspace::prefix` to range over every
    /// scope of one file (e.g. when checking what retention rules
    /// govern a single artifact).
    pub fn file_prefix(actor_id: Uuid, file_id: Uuid) -> [u8; 32] {
        let mut prefix = [0u8; 32];
        prefix[..16].copy_from_slice(actor_id.as_bytes());
        prefix[16..].copy_from_slice(file_id.as_bytes());
        prefix
    }

    /// Read the actor id (bytes 0..16) from a full retention-key
    /// byte slice.
    pub fn actor_id_from_bytes(bytes: &[u8]) -> Option<Uuid> {
        if bytes.len() < 16 {
            return None;
        }
        let mut b = [0u8; 16];
        b.copy_from_slice(&bytes[..16]);
        Some(Uuid::from_bytes(b))
    }

    /// Read the file id (bytes 16..32) from a full retention-key
    /// byte slice.
    pub fn file_id_from_bytes(bytes: &[u8]) -> Option<Uuid> {
        if bytes.len() < 32 {
            return None;
        }
        let mut b = [0u8; 16];
        b.copy_from_slice(&bytes[16..32]);
        Some(Uuid::from_bytes(b))
    }
}

/// Pinned byte map for [`RetentionScope`]. These bytes are part
/// of the on-disk format — **do not renumber**. New variants get
/// the next free byte.
const SCOPE_BYTE_ORIGINAL_CONTENT: u8 = 0x01;
const SCOPE_BYTE_REDACTED_OUTPUT: u8 = 0x02;
const SCOPE_BYTE_AUDIT_LOGS: u8 = 0x03;

fn scope_to_byte(scope: RetentionScope) -> u8 {
    match scope {
        RetentionScope::OriginalContent => SCOPE_BYTE_ORIGINAL_CONTENT,
        RetentionScope::RedactedOutput => SCOPE_BYTE_REDACTED_OUTPUT,
        RetentionScope::AuditLogs => SCOPE_BYTE_AUDIT_LOGS,
        // `RetentionScope` is `#[non_exhaustive]`. A future
        // variant reaching this match in an older binary would
        // silently mis-encode; the workspace builds against one
        // version of `nvisy-core` so this arm is a
        // forward-compat hatch — pick `0x00`, which no valid
        // variant claims.
        _ => 0x00,
    }
}
