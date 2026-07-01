//! Composite keys for the fjall keyspaces.
//!
//! Every key type here is a `#[repr(transparent)]`-style newtype
//! over a big-endian byte array so fjall's lex ordering matches
//! the natural prefix order. Each file owns one key shape:
//!
//! - [`CompositeKey`] — `(actor_id, resource_id)`, 32 bytes.
//!   2-component actor scoping.
//! - [`VersionedKey`] — `(actor, resource, version)`, variable
//!   length. Per-version resource storage (policies, contexts);
//!   the version is `(major, minor, patch)` big-endian u64s so
//!   lex order matches semver order.
//! - [`RunDocKey`] — `(actor, run, doc)`, 48 bytes. Per-document
//!   run bodies; prefix scan by `(actor, run)` returns every
//!   document in a run.
//! - [`RetentionKey`] — `(actor, file, scope_byte)`, 33 bytes.
//!   Retention schedule; prefix scan by `(actor, file)` returns
//!   every retention row for one file.
//!
//! The reverse index for active-file references has no
//! separate key type — [`crate::retention::active_refs::ActiveFileRef`]
//! is both the parsed row and the encoded key, via `to_bytes()`.

mod composite;
mod retention;
mod run_doc;
mod versioned;

pub(crate) use self::composite::CompositeKey;
pub(crate) use self::retention::RetentionKey;
pub(crate) use self::run_doc::RunDocKey;
pub(crate) use self::versioned::VersionedKey;
