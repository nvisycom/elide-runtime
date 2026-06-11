//! Codec core contracts, grouped by concern:
//!
//! - `format` — *what kind of thing a codec is*. [`FormatId`],
//!   [`Format`] descriptor, [`ModalityKind`] tag.
//! - `handle` — *what a handle exposes*. [`Handler`] (base
//!   trait), [`Codable`] (modality marker → [`ModalityKind`] bridge),
//!   [`Handle<M>`] (per-modality capability surface), [`Chunk<M>`]
//!   payload, [`HandleId`] for embeds, [`EmbeddedHandles`] for
//!   rich-format children.
//! - `loader` — *how raw bytes become a handle*. [`Loader<M>`]
//!   (per-modality decoder), [`ErasedLoader`] (object-safe surface
//!   the registry stores), and a crate-internal `erase` helper that
//!   bridges typed loaders into [`Format::new`].
//! - `registry` — *the lookup engine*. [`CodecRegistry`] indexes
//!   [`Format`]s by id, extension, and content type, and decodes
//!   bytes through the matching loader.
//!
//! Concrete format implementations live in `crate::handler::*`;
//! their `impl Codable for X` blocks live next to the per-modality
//! markers they specialise.

mod format;
mod handle;
mod loader;
mod registry;

pub use self::format::{Format, FormatId, ModalityKind};
pub use self::handle::{Chunk, Codable, EmbeddedHandles, Handle, HandleId, Handler};
pub(crate) use self::loader::erase;
pub use self::loader::{ErasedLoader, Loader};
pub use self::registry::CodecRegistry;
