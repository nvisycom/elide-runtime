//! Codec core contracts: the per-modality trait surfaces and the
//! supporting types (locations, spans, redaction collections) those
//! traits reference.
//!
//! - [`Codable`] — per-modality wire-type associated types.
//! - [`Handle<M>`] — per-modality capability trait.
//! - [`Located<M, D>`] / [`LocationStream<M>`] — per-modality
//!   location streaming primitives. `Located<M>` is the bare
//!   location form (`Located<M, ()>`); `Located<M, D>` is the same
//!   record with content data attached.
//! - [`Redactions<M, R>`] + [`ConflictPolicy`] — per-modality
//!   redaction collections.
//!
//! Base traits ([`Handler`], [`Loader`]) live in [`crate::handler`],
//! since they're spelled out next to the concrete per-modality wire
//! types implementing them. Concrete per-modality wire types
//! (`TextData`, `TextRedaction`, `ImageData`, …) live in
//! [`crate::handler`]; concrete format handlers live in
//! `nvisy-formats`.
//!
//! [`Handler`]: crate::handler::Handler
//! [`Loader`]: crate::handler::Loader

mod handle;
mod located;
mod policy;
mod redactions;
mod stream;

pub use nvisy_ontology::modality::Mergeable;

pub use self::handle::{Codable, Handle};
pub use self::located::Located;
pub use self::policy::{ConflictPolicy, InsertError};
pub use self::redactions::Redactions;
pub use self::stream::LocationStream;
