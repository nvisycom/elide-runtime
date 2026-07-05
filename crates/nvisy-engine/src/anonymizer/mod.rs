//! Compile a [`nvisy_schema::policy::Policy`] set into an
//! [`Anonymizer`] per modality at request time.
//!
//! Policy specs are serialisable and modality-agnostic; elide's
//! [`Anonymizer`]`<M>` is a runtime, modality-typed value that
//! drives actual redaction. This module bridges the two: it walks every
//! enabled rule in precedence order, builds the matching elide
//! operator from the spec, and attaches it to the anonymizer with a
//! predicate built from the rule's selector.
//!
//! ## Layout
//!
//! - `text` / `tabular` consume the text-backed redaction specs
//!   (the full elide built-in vocabulary: Erase, Keep, Mask,
//!   Replace, Hash, Pseudonymize, Encrypt — plus the structural
//!   DropRow / DropColumn on tabular).
//! - `image` handles the image specs (Erase, Keep, Blur, Pixelate,
//!   Blackbox).
//! - `audio` handles the audio specs (Erase, Keep, Silence, Beep).
//!
//! Each per-modality `compile` entry walks `&[Policy]` in
//! precedence order; within each policy, rules are tried in
//! declared order; the first matching rule's operator wins. A
//! policy's `fallback`, if Redact with that modality's arm set,
//! becomes the anonymizer's catch-all.
//!
//! ## Audit decoration
//!
//! Each compiled operator will be wrapped in a thin decorator that
//! stamps the policy/rule attribution onto the audit when the
//! operator runs. The decoration lives outside the per-modality
//! compile helpers — they assemble naked operators today; the audit
//! pass wraps them in a follow-up.
//!
//! [`Anonymizer`]: elide::redaction::Anonymizer

#[cfg(feature = "internal_audio")]
mod audio;
mod dispatch;
#[cfg(feature = "internal_image")]
mod image;
mod selector;
#[cfg(feature = "internal_tabular")]
mod tabular;
mod text;
mod text_op;

#[cfg(feature = "internal_audio")]
pub(crate) use self::audio::{attach_override_audio, attach_policies_audio};
#[cfg(feature = "internal_image")]
pub(crate) use self::image::{attach_override_image, attach_policies_image};
#[cfg(feature = "internal_tabular")]
pub(crate) use self::tabular::{attach_override_tabular, attach_policies_tabular};
pub(crate) use self::text::{attach_override_text, attach_policies_text};
