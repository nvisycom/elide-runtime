//! Per-modality operator vocabularies.
//!
//! [`text`] owns the text-and-tabular wire → runtime bridge: one
//! [`compile_and_attach`] function that matches a
//! [`TextRedaction`] and calls [`Target::attach_with`] with the
//! concrete elide operator. Image and audio each own their own
//! `Op` enum + `attach_to` dispatcher; those two vocabularies
//! don't overlap with text.
//!
//! [`text`]: self::text
//! [`compile_and_attach`]: self::text::compile_and_attach
//! [`Target::attach_with`]: super::compile::Target::attach_with
//! [`TextRedaction`]: elide_governance::redaction::TextRedaction

#[cfg(feature = "internal_audio")]
pub(super) mod audio;
#[cfg(feature = "internal_image")]
pub(super) mod image;
pub(super) mod text;
