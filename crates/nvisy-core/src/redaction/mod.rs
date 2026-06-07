//! Redaction-side primitives shared across the runtime.
//!
//! - [`Anonymizer<M>`] + [`LeakProfile`] — the per-modality
//!   redaction operator trait, plus the leak-profile enum operators
//!   self-report. Built-in implementations (mask, hash, encrypt,
//!   keep, redact, replace) ship in `nvisy-toolkit`; the fake-data
//!   generator ships in `nvisy-fake`.
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
//! The read-side siblings [`TextAt`] / [`DataAt`] live in
//! [`extraction`].
//!
//! [`DataAt`]: crate::extraction::DataAt
//! [`extraction`]: crate::extraction
//! [`Modality::Replacement`]: crate::modality::Modality::Replacement
//! [`TextAt`]: crate::extraction::TextAt

mod anonymizer;
mod deanonymizer;
mod leak_profile;
mod redact_at;
mod redactions;
mod replacement;

pub use self::anonymizer::Anonymizer;
pub use self::deanonymizer::Deanonymizer;
pub use self::leak_profile::LeakProfile;
pub use self::redact_at::RedactAt;
pub use self::redactions::Redactions;
pub use self::replacement::{
    AudioReplacement, ImageReplacement, TabularReplacement, TextReplacement,
};
