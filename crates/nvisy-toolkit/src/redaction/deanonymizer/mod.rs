//! Built-in [`Deanonymizer<M>`] implementations shipped with the
//! toolkit.
//!
//! Each operator recovers the original payload an [`Anonymizer<M>`]
//! wrote. Two recovery shapes ship today (see [`Deanonymizer`]):
//! audit-keyed (no impl yet) and self-contained (e.g. `Decrypt`,
//! gated behind the `encrypt` feature).
//!
//! [`Anonymizer<M>`]: crate::redaction::Anonymizer
//! [`Deanonymizer<M>`]: crate::redaction::Deanonymizer
//! [`Deanonymizer`]: crate::redaction::Deanonymizer

#[cfg(feature = "encrypt")]
#[cfg_attr(docsrs, doc(cfg(feature = "encrypt")))]
mod decrypt;

#[cfg(feature = "encrypt")]
#[cfg_attr(docsrs, doc(cfg(feature = "encrypt")))]
pub use self::decrypt::Decrypt;
