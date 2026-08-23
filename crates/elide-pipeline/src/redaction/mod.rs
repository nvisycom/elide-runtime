//! Redaction: which entities to hide, and how.
//!
//! Compiles a [`elide_governance::PolicyDefinition`] set into an
//! [`Anonymizer`] per modality at request time.
//!
//! Mirrors [`crate::recognition`], which does the same for the
//! other direction: where recognition finds entities, this hides
//! them. The asymmetry is where the configuration comes
//! from — recognition's is deployment-owned and wired once at
//! startup, redaction's arrives per request as policies.
//!
//! PolicyDefinition specs are serialisable and modality-agnostic; elide's
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
//!   Replace, Hash, Pseudonymize, Encrypt: plus the structural
//!   DropRow / DropColumn on tabular).
//! - `image` handles the image specs (Erase, Keep, Blur, Pixelate,
//!   Blackbox).
//! - `audio` handles the audio specs (Erase, Keep, Silence, Beep).
//!
//! Each per-modality `compile` entry walks `&[PolicyDefinition]` in
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
//! compile helpers: they assemble naked operators today; the audit
//! pass wraps them in a follow-up.
//!
//! [`Anonymizer`]: elide::redaction::Anonymizer

mod audio;
mod compile;
mod image;
mod operator;
mod tabular;
mod text;

pub(crate) use self::audio::{attach_override_audio, attach_policies_audio};
pub(crate) use self::image::{attach_override_image, attach_policies_image};
pub(crate) use self::operator::text::TextOperatorContext;
pub(crate) use self::tabular::{attach_override_tabular, attach_policies_tabular};
pub(crate) use self::text::{attach_override_text, attach_policies_text};
