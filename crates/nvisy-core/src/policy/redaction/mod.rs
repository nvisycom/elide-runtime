//! Per-modality redaction operator specs — the closed wire vocabulary
//! a policy author can choose from inside a `redact` rule — plus the
//! [`ModalityRedactions`] map a rule carries to wire one operator
//! per modality, and the [`AnyRedaction`] erasure used at the
//! override boundary.
//!
//! Each modality has its own enum because the operator catalogue
//! differs by modality. Text gets the full elide built-in set
//! ([`Replace`], [`Mask`], [`Hash`], [`Erase`], [`Keep`]) plus a
//! `Custom` escape hatch. Image / Audio / Tabular currently expose
//! only `Custom` — elide ships no built-in operators wired into the
//! policy wire format for those modalities.
//!
//! The split between these enums (the spec) and elide's
//! [`Operator<M>`] trait is intentional: the spec is the
//! serialisable, author-facing wire shape; the engine compiles each
//! variant into the matching runtime operator instance at apply
//! time.
//!
//! [`Replace`]: elide::redaction::operators::Replace
//! [`Mask`]: elide::redaction::operators::Mask
//! [`Hash`]: elide::redaction::operators::Hash
//! [`Erase`]: elide::redaction::operators::Erase
//! [`Keep`]: elide::redaction::operators::Keep
//! [`Operator<M>`]: elide_core::operator::Operator

mod any;
mod audio;
mod image;
mod tabular;
mod text;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

pub use self::any::{AnyRedaction, RedactionModality};
pub use self::audio::AudioRedaction;
pub use self::image::ImageRedaction;
pub use self::tabular::TabularRedaction;
pub use self::text::{HashAlgorithm, TextRedaction};

/// Per-modality operator specs carried by a `redact` rule.
///
/// A single rule can name an operator for every modality the
/// workspace supports. At apply time the redaction phase picks the
/// operator matching the entity's modality (engine reads the right
/// field directly — `redactions.text.as_ref()` etc.); modalities
/// the rule didn't cover fall through to the deployment-wide
/// default, and entities with no operator from either source are
/// skipped.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub struct ModalityRedactions {
    /// Operator for text-modality entities.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub text: Option<TextRedaction>,
    /// Operator for tabular-modality entities.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tabular: Option<TabularRedaction>,
    /// Operator for image-modality entities.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub image: Option<ImageRedaction>,
    /// Operator for audio-modality entities.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub audio: Option<AudioRedaction>,
}

impl ModalityRedactions {
    /// `true` when no operator is set for any modality. A rule whose
    /// `redact` field is empty after deserialisation is an author
    /// error — the request validator rejects it.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.text.is_none()
            && self.tabular.is_none()
            && self.image.is_none()
            && self.audio.is_none()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_when_no_modality_set() {
        let r = ModalityRedactions::default();
        assert!(r.is_empty());
    }

    #[test]
    fn non_empty_when_any_modality_set() {
        let r = ModalityRedactions {
            text: Some(TextRedaction::Erase),
            ..Default::default()
        };
        assert!(!r.is_empty());
    }
}
