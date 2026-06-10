//! Ingestion: the read-side edge of a toolkit pipeline.
//!
//! Re-exports the codec front door so a toolkit-only consumer reaches
//! the canonical "load bytes → typed handle" pipeline without
//! pulling `nvisy-codec` into its own dep list:
//!
//! - [`CodecRegistry`] is the entry point — call
//!   [`CodecRegistry::with_builtin`] to get every shipped format the
//!   active features compile in, then [`decode_from_memory`] to
//!   commit bytes to a typed handle.
//! - [`DocumentHandle<M>`] is the typed handle. It implements the
//!   resolver traits ([`TextAt`], [`DataAt`]) plus [`RedactAt`] that
//!   detection / deduplication / redaction phases bound on, so the
//!   handle itself is the buffer the pipeline operates on.
//!
//! The per-modality `*At` impls on `DocumentHandle<M>` in
//! `nvisy-codec` are cfg-gated on `internal_text` / `internal_tabular`
//! / `internal_image` / `internal_audio`. Each toolkit feature
//! (`text` / `tabular` / `image` / `audio` / `rich`) forwards to the
//! matching codec umbrella, which in turn activates the right
//! `internal_*` flags.
//!
//! [`CodecRegistry`]: nvisy_codec::CodecRegistry
//! [`CodecRegistry::with_builtin`]: nvisy_codec::CodecRegistry::with_builtin
//! [`decode_from_memory`]: nvisy_codec::CodecRegistry::decode_from_memory
//! [`DocumentHandle<M>`]: nvisy_codec::document::DocumentHandle
//! [`TextAt`]: nvisy_core::extraction::TextAt
//! [`DataAt`]: nvisy_core::extraction::DataAt
//! [`RedactAt`]: nvisy_core::redaction::RedactAt

pub use nvisy_codec::CodecRegistry;
pub use nvisy_codec::document::DocumentHandle;
