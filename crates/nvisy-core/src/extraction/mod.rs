//! Extraction-side primitives shared across the runtime.
//!
//! Two pieces today:
//!
//! - [`ValueAt`] — the trait every extraction-aware consumer (dedup
//!   layer, validation check, redaction strategy binding) bounds on to
//!   read source text at a per-modality location.
//! - [`Extractor`] — the per-modality extraction trait every backend
//!   implements. Symmetric to [`EntityRecognizer`][crate::EntityRecognizer]
//!   on the producer side.
//!
//! [`Extractor::extraction`] returns the value stamped into the
//! document's per-modality metadata; the enum lives next to its
//! modality type in [`crate::modality`].

mod extractor;
mod value_at;

pub use self::extractor::Extractor;
pub use self::value_at::ValueAt;
