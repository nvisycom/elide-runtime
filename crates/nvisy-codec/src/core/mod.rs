//! Codec core contracts: per-modality trait surfaces, format identity,
//! and the registry that composes them.
//!
//! - [`Codable`] — per-modality wire-type associated types.
//! - [`Handle<M>`] — per-modality capability trait (streaming +
//!   random-access reads + redactions + offset lifting).
//! - [`EmbeddedHandles`] — rich-format embedded-child lookup.
//! - [`Chunk<M>`] — one unit yielded by [`Handle::next_chunk`].
//! - [`HandleId`] — stable identifier for embedded child handles.
//! - [`FormatId`] — stable identifier for a registered format.
//! - [`CodecRegistry`] — extension/content-type → [`Format`] lookup +
//!   decode dispatch.
//! - [`Format`] — descriptor a [`CodecRegistry`] indexes.
//! - [`ErasedLoader`] + [`LoaderAdapter`] — object-safe loader surface
//!   adapting per-modality [`Loader<M>`] impls. The adapter erases
//!   `M` into [`UntypedDocumentHandle`] via the auto-derived
//!   `From<DocumentHandle<M>>` impls on the enum.
//! - [`Handler`] — base trait every format handler implements.
//! - [`Loader<M>`] — per-modality decoder the registry composes.
//!
//! The `(location, replacement)` pair list passed to
//! [`Handle::redact`] is [`Redactions<M>`]; the per-modality
//! replacement enum is in [`redaction`] — codec depends on core, not
//! the reverse.
//!
//! Concrete format implementations and their `impl Codable for X`
//! blocks live in the per-modality top-level modules (`crate::text`,
//! `crate::image`, `crate::audio`, `crate::tabular`, `crate::rich`).
//!
//! [`Redactions<M>`]: nvisy_core::redaction::Redactions
//! [`redaction`]: nvisy_core::redaction
//! [`UntypedDocumentHandle`]: crate::document::UntypedDocumentHandle

mod format;
mod handle;
mod handler;
mod modality;
mod registry;

pub use self::format::FormatId;
pub use self::handle::{Chunk, Codable, EmbeddedHandles, Handle, HandleId};
pub use self::handler::{Handler, Loader};
pub use self::modality::ModalityKind;
pub use self::registry::{CodecRegistry, ErasedLoader, Format, LoaderAdapter};
