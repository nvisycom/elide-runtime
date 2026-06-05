//! Redaction-side primitives shared across the runtime.
//!
//! - [`Redactable`] — extension trait on [`Modality`] naming the
//!   per-modality replacement value.
//! - [`TextReplacement`], [`ImageReplacement`], [`AudioReplacement`],
//!   [`TabularReplacement`] — what an anonymizer emits at the
//!   entity's location.
//!
//! The producer-side anonymizer trait (and its built-ins) lives in
//! `nvisy-toolkit`; the codec-side write-back trait
//! ([`crate::extraction::RedactAt`]) lives next to its read-side
//! siblings in [`crate::extraction`].
//!
//! [`Modality`]: crate::modality::Modality

mod redactable;
mod replacement;

pub use self::redactable::Redactable;
pub use self::replacement::{
    AudioReplacement, ImageReplacement, TabularReplacement, TextReplacement,
};
