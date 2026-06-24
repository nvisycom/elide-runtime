//! Compile a [`nvisy_core::policy::Policy`] set into an
//! [`elide::Anonymizer`] per modality at request time.
//!
//! Policy specs are serialisable and modality-agnostic; elide's
//! [`Anonymizer<M>`] is a runtime, modality-typed value that drives
//! actual redaction. This module bridges the two: it walks every
//! enabled rule in precedence order, builds the matching elide
//! operator from the spec, and attaches it to the anonymizer with a
//! predicate built from the rule's selector.
//!
//! ## Layout
//!
//! - [`text`] / [`tabular`] consume the [`TextBacked`] redaction
//!   specs (the full elide built-in vocabulary: Erase, Keep, Mask,
//!   Replace, Hash, Pseudonymize, Encrypt — plus the structural
//!   DropRow / DropColumn on tabular).
//! - [`image`] handles the image specs (Erase, Keep, Blur,
//!   Pixelate, Blackbox).
//! - [`audio`] handles the audio specs (Erase, Keep, Silence,
//!   Beep).
//!
//! Each per-modality `compile` entry walks `&[Policy]` in
//! precedence order; within each policy, rules are tried in
//! declared order; the first matching rule's operator wins. Each
//! policy's `default_action`, if Redact with that modality's arm
//! set, becomes the anonymizer's fallback.
//!
//! ## Audit decoration
//!
//! Each compiled operator will be wrapped in a thin decorator that
//! stamps [`PolicyDecisionRef`] onto the audit when the operator
//! runs. The decoration lives outside the per-modality compile
//! helpers — they assemble naked operators today; the audit pass
//! wraps them in a follow-up.
//!
//! [`TextBacked`]: elide_core::modality::TextBacked
//! [`PolicyDecisionRef`]: nvisy_core::policy::PolicyDecisionRef

mod audio;
mod image;
mod selector;
mod tabular;
mod text;

pub use self::audio::compile_audio;
pub use self::image::compile_image;
pub use self::tabular::compile_tabular;
pub use self::text::compile_text;

pub(crate) use self::audio::{attach_override_audio, attach_policies_audio};
pub(crate) use self::image::{attach_override_image, attach_policies_image};
pub(crate) use self::tabular::{attach_override_tabular, attach_policies_tabular};
pub(crate) use self::text::{attach_override_text, attach_policies_text};
