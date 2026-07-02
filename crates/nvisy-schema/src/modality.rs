//! Modality-specific runtime types re-exported from
//! `elide_core::modality`.
//!
//! Wire types on [`policy`] (audio redaction operators) carry
//! [`audio::Waveform`] directly.
//!
//! [`policy`]: crate::policy

pub mod audio {
    pub use elide_core::modality::audio::Waveform;
}
