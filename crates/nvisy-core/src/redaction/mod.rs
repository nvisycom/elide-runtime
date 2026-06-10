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
//!   (codec's `DecodedBuffer`, engine's `DocumentTree`).
//! - [`Deanonymizer`] — audit-keyed inverse of `Anonymizer`.
//!   Implemented by recoverable wrappers that persist the original
//!   at apply-time keyed on `entity.id`.
//! - [`Store`] — pluggable token vault keyed on opaque strings.
//!   Backends (in-memory, key-value) ship in `nvisy-toolkit`.
//! - [`Memoized`] — wraps any inner `Anonymizer` and caches its
//!   output so the same payload always gets the same replacement.
//!   No recovery; preserves the inner's `LeakProfile`.
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
mod memoized;
mod redact_at;
mod redactions;
mod replacement;
mod store;

pub use self::anonymizer::Anonymizer;
pub use self::deanonymizer::Deanonymizer;
pub use self::leak_profile::LeakProfile;
pub use self::memoized::Memoized;
pub use self::redact_at::RedactAt;
pub use self::redactions::Redactions;
pub use self::replacement::{
    AudioReplacement, ImageReplacement, TabularReplacement, TextReplacement,
};
pub use self::store::Store;
