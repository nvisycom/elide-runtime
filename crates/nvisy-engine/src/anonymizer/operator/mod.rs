//! Per-modality operator vocabularies.
//!
//! Each submodule owns the `Op` enum plus a wire → runtime
//! conversion — `From<&{Image,Audio}Redaction>` (infallible) or
//! `TryFrom<&TextRedaction>` (fallible; text has stateful
//! operators that aren't wired yet). Per-modality entry files in
//! [`super`] dispatch a converted `Op` onto a
//! [`super::compile::Target`] via `Op::attach_to`.
//!
//! Text and tabular share the text vocabulary — cells in a
//! tabular are `TextBacked` in elide, so every text operator
//! also implements `Operator<Tabular>`. Image and audio each own
//! their vocabulary; the operators don't cross modalities.

#[cfg(feature = "internal_audio")]
pub(super) mod audio;
#[cfg(feature = "internal_image")]
pub(super) mod image;
pub(super) mod text;
