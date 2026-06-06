//! Redaction-side primitives shared across the runtime.
//!
//! - [`TextReplacement`], [`ImageReplacement`], [`AudioReplacement`],
//!   [`TabularReplacement`] — what an anonymizer emits at the
//!   entity's location. Each modality binds one of these via
//!   [`Modality::Replacement`].
//! - [`Redactions<M>`] — typed batch of `(M::Location, M::Replacement)`
//!   pairs handed from the producer side (anonymizer / registry) to
//!   the write-back side.
//! - [`RedactAt<M>`] — the write-back trait every sink implements
//!   (toolkit's `MemoryBuffer`, document's `DocumentTree`).
//!
//! The producer-side anonymizer trait (and its built-ins) lives in
//! `nvisy-toolkit`; the read-side siblings [`TextAt`] / [`DataAt`]
//! live in [`extraction`].
//!
//! [`DataAt`]: crate::extraction::DataAt
//! [`extraction`]: crate::extraction
//! [`Modality::Replacement`]: crate::modality::Modality::Replacement
//! [`TextAt`]: crate::extraction::TextAt

mod redact_at;
mod redactions;
mod replacement;

pub use self::redact_at::RedactAt;
pub use self::redactions::Redactions;
pub use self::replacement::{
    AudioReplacement, ImageReplacement, TabularReplacement, TextReplacement,
};
