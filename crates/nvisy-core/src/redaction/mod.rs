//! Redaction-side primitives shared across the runtime.
//!
//! - [`Redactable`] — extension trait on [`Modality`] naming the
//!   per-modality replacement value.
//! - [`TextReplacement`], [`ImageReplacement`], [`AudioReplacement`],
//!   [`TabularReplacement`] — what an anonymizer emits at the
//!   entity's location.
//! - [`Redactions<M>`] — typed batch of `(M::Location, M::Replacement)`
//!   pairs handed from the producer side (anonymizer / registry) to
//!   the write-back side.
//! - [`RedactAt<M>`] — the write-back trait every sink implements
//!   (toolkit's `MemoryBuffer`, document's `DocumentTree`).
//!
//! The producer-side anonymizer trait (and its built-ins) lives in
//! `nvisy-toolkit`; the read-side siblings [`crate::extraction::TextAt`]
//! / [`crate::extraction::DataAt`] live in [`crate::extraction`].
//!
//! [`Modality`]: crate::modality::Modality

mod redact_at;
mod redactable;
mod redactions;
mod replacement;

pub use self::redact_at::RedactAt;
pub use self::redactable::Redactable;
pub use self::redactions::Redactions;
pub use self::replacement::{
    AudioReplacement, ImageReplacement, TabularReplacement, TextReplacement,
};
