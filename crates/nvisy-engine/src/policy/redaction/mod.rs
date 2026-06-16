//! Per-modality redaction operator specs — the closed wire vocabulary
//! a policy author can choose from inside a `redact` rule — plus the
//! [`ModalityRedactions`] map a rule carries to wire one operator per
//! modality, and the [`AnyRedaction`] erasure used at the dynamic
//! override boundary.
//!
//! Each modality has its own enum because the operator catalogue
//! differs by modality. Text gets the full toolkit built-in set
//! (`Replace`, `Mask`, `Hash`, `Redact`, `Keep`) plus a `Custom`
//! escape hatch. Image / Audio / Tabular currently expose only
//! `Custom` — the toolkit ships no built-in operators for those
//! modalities yet.
//!
//! The split between these enums (the spec) and the toolkit's
//! [`Anonymizer<M>`] trait is intentional: the spec is the
//! serialisable, author-facing wire shape; instantiating it at
//! apply time produces the runtime operator instance.
//!
//! [`Anonymizer<M>`]: nvisy_toolkit::redaction::Anonymizer

mod any;
mod audio;
mod image;
mod tabular;
mod text;

use nvisy_core::modality::{Audio, Image, Tabular, Text};
pub use nvisy_toolkit::redaction::anonymizer::HashAlgorithm;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

pub use self::any::AnyRedaction;
pub use self::audio::AudioRedaction;
pub use self::image::ImageRedaction;
pub use self::tabular::TabularRedaction;
pub use self::text::TextRedaction;
use crate::modality::DocumentModality;

/// Per-modality operator specs carried by a `redact` rule.
///
/// A single rule can name an operator for every modality the
/// workspace supports. At apply time the redaction phase picks the
/// operator matching the entity's modality via
/// [`ModalityRedactions::operator_for`]; modalities the rule didn't
/// cover fall through to the deployment-wide default
/// (`RedactionConfig::default_operators`), and entities with no
/// operator from either source are skipped.
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
    /// `true` when no operator is set for any modality. A rule
    /// whose `redact` field is empty after deserialisation is an
    /// author error — the request validator rejects it.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.text.is_none()
            && self.tabular.is_none()
            && self.image.is_none()
            && self.audio.is_none()
    }

    /// Borrow the operator spec for modality `M`, if one is set.
    ///
    /// The apply phase uses this to pick the right operator for an
    /// entity. Returns `None` when this rule didn't cover the
    /// modality.
    #[must_use]
    pub fn operator_for<M: ProjectRedaction>(&self) -> Option<&M::Redaction> {
        M::project(self)
    }
}

/// Sealed projection trait that picks the typed `*Redaction` field
/// from a [`ModalityRedactions`] for the implementing modality.
///
/// One impl per workspace modality; not extensible by downstream
/// crates today. When user-defined modalities land, this becomes
/// the seam to widen.
pub trait ProjectRedaction: DocumentModality {
    /// Borrow this modality's operator spec out of `redactions`.
    fn project(redactions: &ModalityRedactions) -> Option<&Self::Redaction>;
}

impl ProjectRedaction for Text {
    fn project(r: &ModalityRedactions) -> Option<&Self::Redaction> {
        r.text.as_ref()
    }
}

impl ProjectRedaction for Tabular {
    fn project(r: &ModalityRedactions) -> Option<&Self::Redaction> {
        r.tabular.as_ref()
    }
}

impl ProjectRedaction for Image {
    fn project(r: &ModalityRedactions) -> Option<&Self::Redaction> {
        r.image.as_ref()
    }
}

impl ProjectRedaction for Audio {
    fn project(r: &ModalityRedactions) -> Option<&Self::Redaction> {
        r.audio.as_ref()
    }
}

#[cfg(test)]
mod tests {
    use nvisy_toolkit::redaction::anonymizer::HashAlgorithm;

    use super::*;

    #[test]
    fn empty_when_no_modality_set() {
        let r = ModalityRedactions::default();
        assert!(r.is_empty());
    }

    #[test]
    fn non_empty_when_any_modality_set() {
        let r = ModalityRedactions {
            text: Some(TextRedaction::Redact),
            ..Default::default()
        };
        assert!(!r.is_empty());
    }

    #[test]
    fn operator_for_text_round_trips() {
        let r = ModalityRedactions {
            text: Some(TextRedaction::Hash {
                algorithm: HashAlgorithm::Sha256,
                salt: None,
            }),
            ..Default::default()
        };
        let op = r.operator_for::<Text>().expect("text operator set");
        assert!(matches!(op, TextRedaction::Hash { .. }));
        assert!(r.operator_for::<Image>().is_none());
    }
}
