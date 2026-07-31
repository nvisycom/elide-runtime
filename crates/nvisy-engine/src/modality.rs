//! Modality markers and traits parameterising `Entity<M>` and
//! `EntityRecord<M>`.
//!
//! [`Modality`] is the trait every marker implements; [`Text`],
//! [`Tabular`], [`Image`], and [`Audio`] are the four markers.
//! [`Waveform`] appears directly on audio redaction operators
//! in [`crate::policy`].

pub use nvisy_schema::modality::{
    Audio, Image, Modality, ModalityData, ModalityLocation, ModalityReplacement, Tabular, Text,
    Waveform,
};
