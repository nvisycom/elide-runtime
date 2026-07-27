//! Modality markers and traits parameterising `Entity<M>` and
//! `EntityRecord<M>`.
//!
//! [`Modality`] is the trait every marker implements; [`Text`],
//! [`Tabular`], [`Image`], and [`Audio`] are the four markers.
//! [`Waveform`] appears directly on audio redaction operators in
//! [`policy`].
//!
//! [`policy`]: crate::policy

pub use elide_core::modality::audio::{Audio, Waveform};
pub use elide_core::modality::image::Image;
pub use elide_core::modality::tabular::Tabular;
pub use elide_core::modality::text::Text;
pub use elide_core::modality::{Modality, ModalityData, ModalityLocation, ModalityReplacement};
