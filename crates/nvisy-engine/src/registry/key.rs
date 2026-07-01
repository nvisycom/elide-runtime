//! Composite keys for the fjall keyspaces.
//!
//! Three key shapes, all big-endian byte-encoded so fjall's lex
//! ordering matches the natural prefix order:
//!
//! - [`CompositeKey`] — `(actor_id, resource_id)`, 32 bytes. Existing
//!   2-component scoping inherited from the salvaged registry.
//! - [`VersionedKey`] — `(actor_id, resource_id, version_serialised)`,
//!   variable length. For per-version resource storage (policies,
//!   contexts). Prefix scan by `(actor, id)` returns every version of
//!   the same logical resource in lex order.
//! - [`RunDocKey`] — `(actor_id, run_id, doc_id)`, 48 bytes. For
//!   per-document run bodies. Prefix scan by `(actor, run)` returns
//!   every document in a run.
//! - [`RetentionKey`] — `(actor_id, file_id, scope_byte)`, 33
//!   bytes. For the retention schedule. Prefix scan by
//!   `(actor, file)` returns every retention row for one file.
//! - [`ActiveFileRefKey`] — `(actor_id, file_id, run_id)`, 48
//!   bytes. Reverse index for the sweeper's active-run gate:
//!   prefix scan by `(actor, file)` returns every non-terminal
//!   run still referencing that file. Point deletes at run
//!   terminal transitions synthesise each key directly from
//!   the run's `document_ids`.

use derive_more::{AsRef, Deref, From};
use nvisy_core::policy::RetentionScope;
use semver::Version;
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

/// 33-byte retention key:
/// `[actor: 16][file: 16][scope: 1]`.
///
/// `scope` is encoded via a stable byte map ([`scope_to_byte`] /
/// [`scope_from_byte`]) — the on-disk bytes are part of the
/// format, so the mapping must not be renumbered. Prefix scan by
/// `(actor, file)` returns every retention row for one file.
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

/// 48-byte active-file-reference key:
/// `[actor_id: 16][file_id: 16][run_id: 16]`.
///
/// The reverse index for the sweeper's active-run gate: any
/// row surviving the `(actor, file)` prefix scan means some
/// non-terminal run still references the file, and the sweeper
/// defers.
#[derive(Clone, Copy, Deref, AsRef, From)]
#[as_ref(forward)]
pub(crate) struct ActiveFileRefKey(#[deref] [u8; 48]);

impl ActiveFileRefKey {
    /// Build from `(actor, file, run)`.
    pub fn new(actor_id: Uuid, file_id: Uuid, run_id: Uuid) -> Self {
        let mut key = [0u8; 48];
        key[..16].copy_from_slice(actor_id.as_bytes());
        key[16..32].copy_from_slice(file_id.as_bytes());
        key[32..].copy_from_slice(run_id.as_bytes());
        Self(key)
    }

    /// Prefix bytes for "every active run referencing
    /// `(actor, file)`": 32 bytes. The gate's read path.
    pub fn file_prefix(actor_id: Uuid, file_id: Uuid) -> [u8; 32] {
        let mut prefix = [0u8; 32];
        prefix[..16].copy_from_slice(actor_id.as_bytes());
        prefix[16..].copy_from_slice(file_id.as_bytes());
        prefix
    }

    /// Parse a full active-ref key back to
    /// `(actor_id, file_id, run_id)`. Returns `None` when
    /// `bytes` isn't 48 bytes long — the startup reap uses this
    /// to reject a malformed row rather than treating it as a
    /// ghost.
    pub fn parse(bytes: &[u8]) -> Option<(Uuid, Uuid, Uuid)> {
        if bytes.len() != 48 {
            return None;
        }
        let mut a = [0u8; 16];
        let mut f = [0u8; 16];
        let mut r = [0u8; 16];
        a.copy_from_slice(&bytes[..16]);
        f.copy_from_slice(&bytes[16..32]);
        r.copy_from_slice(&bytes[32..]);
        Some((
            Uuid::from_bytes(a),
            Uuid::from_bytes(f),
            Uuid::from_bytes(r),
        ))
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
