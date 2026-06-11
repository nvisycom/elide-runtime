//! Codec core contracts, grouped by concern:
//!
//! - `format` — *what kind of thing a codec is*. [`FormatId`],
//!   [`Format`] descriptor.
//! - `handler` — *what a handler exposes*. [`Handler<M>`]
//!   (per-modality capability surface — identify, encode, stream,
//!   read, redact, lift), [`Chunk<M>`] payload.
//! - `loader` — *how raw bytes become a handle*. [`Loader<M>`]
//!   (per-modality decoder). The registry-side erasure machinery
//!   (`ErasedLoader` trait, `erase` helper) is crate-internal and
//!   wired through [`Format::new`] / [`Format::decode`].
//! - `registry` — *the lookup engine*. [`CodecRegistry`] indexes
//!   [`Format`]s by id, extension, and content type, and decodes
//!   bytes through the matching loader.
//!
//! Concrete format implementations live in `crate::handler::*`.

mod format;
mod handler;
mod loader;
mod registry;

pub use self::format::{Format, FormatId};
pub use self::handler::{Chunk, Handler};
pub use self::loader::Loader;
pub(crate) use self::loader::{ErasedLoader, erase};
pub use self::registry::CodecRegistry;
