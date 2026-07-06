//! Per-modality operator vocabularies.
//!
//! Each submodule owns the `Op` enum + `build` fn that maps a
//! wire spec (`{Text,Image,Audio}Redaction`) to elide's concrete
//! operator types (e.g. `Erase`, `Blur`, `Beep`). Per-modality
//! entry files in [`super`] dispatch a built `Op` onto a
//! [`super::compile::Target`] via the `Op::attach_to` method.
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
