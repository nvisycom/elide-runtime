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
//! [`Operator<M>`]: elide_core::redaction::Operator

mod any;
mod audio;
mod image;
mod tabular;
mod text;

use elide_core::modality::audio::Audio;
use elide_core::modality::image::Image;
use elide_core::modality::tabular::Tabular;
use elide_core::modality::text::Text;
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
/// operator matching the entity's modality via
/// [`ModalityRedactions::operator_for`]; modalities the rule didn't
/// cover fall through to the deployment-wide default, and entities
/// with no operator from either source are skipped.
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

    /// Borrow the operator spec for modality `M`, if one is set.
    ///
    /// The apply phase uses this to pick the right operator for an
    /// entity. Returns `None` when this rule didn't cover the
    /// modality.
    #[must_use]
    pub fn operator_for<M: PolicyModality>(&self) -> Option<&M::Redaction> {
        M::project(self)
    }
}

/// Runtime-side extension of [`elide_core::modality::Modality`] that
/// pairs each modality with the policy redaction spec it can carry.
///
/// One impl per modality elide ships. Not extensible by downstream
/// crates today; when user-defined modalities land in elide, this is
/// the seam to widen.
pub trait PolicyModality: elide_core::modality::Modality {
    /// The policy spec enum a `redact` rule names for this modality.
    type Redaction;

    /// Borrow this modality's operator spec out of `redactions`.
    fn project(redactions: &ModalityRedactions) -> Option<&Self::Redaction>;
}

impl PolicyModality for Text {
    type Redaction = TextRedaction;
    fn project(r: &ModalityRedactions) -> Option<&Self::Redaction> {
        r.text.as_ref()
    }
}

impl PolicyModality for Tabular {
    type Redaction = TabularRedaction;
    fn project(r: &ModalityRedactions) -> Option<&Self::Redaction> {
        r.tabular.as_ref()
    }
}

impl PolicyModality for Image {
    type Redaction = ImageRedaction;
    fn project(r: &ModalityRedactions) -> Option<&Self::Redaction> {
        r.image.as_ref()
    }
}

impl PolicyModality for Audio {
    type Redaction = AudioRedaction;
    fn project(r: &ModalityRedactions) -> Option<&Self::Redaction> {
        r.audio.as_ref()
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
