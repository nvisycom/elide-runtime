//! Per-modality redaction operator specs: the closed wire vocabulary a policy.
//!
//! Author can choose from inside a `redact` rule, plus the
//! [`ModalityRedactions`] map a rule carries to wire one operator
//! per modality.
//!
//! Each modality has its own enum because the operator catalogue
//! differs by modality. Text carries the full elide built-in set
//! ([`Erase`], [`Keep`], [`Mask`], [`Replace`], [`Hash`], [`Fake`],
//! [`Pseudonymize`], [`Encrypt`], [`HmacHash`], [`Truncate`],
//! [`Clamp`], [`GeneralizeDate`], and the [`WithFallback`] wrapper).
//! Image, audio, and tabular each carry their own operator sets
//! (blur/pixelate/blackbox for image, silence/beep for audio,
//! drop-row/drop-column plus cell-level text ops for tabular).
//!
//! The split between these enums (the spec) and elide's
//! [`Operator<M>`] trait is intentional: the spec is the
//! serialisable, author-facing wire shape; the engine compiles each
//! variant into the matching runtime operator instance at apply
//! time.
//!
//! [`Erase`]: https://docs.rs/elide/latest/elide/redaction/operators/struct.Erase.html
//! [`Keep`]: https://docs.rs/elide/latest/elide/redaction/operators/struct.Keep.html
//! [`Mask`]: https://docs.rs/elide/latest/elide/redaction/operators/struct.Mask.html
//! [`Replace`]: https://docs.rs/elide/latest/elide/redaction/operators/struct.Replace.html
//! [`Hash`]: https://docs.rs/elide/latest/elide/redaction/operators/struct.Sha2Hash.html
//! [`Fake`]: https://docs.rs/elide_fake/latest/elide_fake/struct.Fake.html
//! [`Pseudonymize`]: https://docs.rs/elide/latest/elide/redaction/operators/struct.Pseudonymize.html
//! [`Encrypt`]: https://docs.rs/elide/latest/elide/redaction/operators/struct.AesEncrypt.html
//! [`HmacHash`]: https://docs.rs/elide/latest/elide/redaction/operators/struct.HmacHash.html
//! [`Truncate`]: https://docs.rs/elide/latest/elide/redaction/operators/struct.Truncate.html
//! [`Clamp`]: https://docs.rs/elide/latest/elide/redaction/operators/struct.Clamp.html
//! [`GeneralizeDate`]: https://docs.rs/elide/latest/elide/redaction/operators/struct.GeneralizeDate.html
//! [`WithFallback`]: https://docs.rs/elide/latest/elide/redaction/operators/struct.WithFallback.html
//! [`Operator<M>`]: elide_core::redaction::Operator

mod audio;
mod image;
mod tabular;
mod text;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

pub use self::audio::AudioRedaction;
pub use self::image::ImageRedaction;
pub use self::tabular::TabularRedaction;
pub use self::text::{ClampBucket, TerminalFallback, TextRedaction};

/// Per-modality operator specs carried by a `redact` rule.
///
/// A single rule can name an operator for every modality the
/// workspace supports. At apply time the redaction phase picks the
/// operator matching the entity's modality (engine reads the right
/// field directly via `redactions.text.as_ref()` etc.); modalities
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
    /// Shortcut for the common "text-only" case:
    /// `ModalityRedactions::text(TextRedaction::Erase)` builds an
    /// action that fires on text entities and leaves every other
    /// modality untouched. Every regulatory template constructor
    /// uses this shape; expressing it as a builder saves the
    /// `..Default::default()` boilerplate at every call site.
    #[must_use]
    pub fn text(spec: TextRedaction) -> Self {
        Self {
            text: Some(spec),
            ..Self::default()
        }
    }

    /// A text operator applied wherever text lives: the text
    /// modality, and tabular cells, which elide backs with text.
    ///
    /// The shape a records-oriented policy wants. A rule built with
    /// [`text`](Self::text) alone matches a tabular entity and
    /// attaches nothing, so the cell passes through unredacted with
    /// no error — this closes that gap for the operators tabular
    /// shares.
    ///
    /// Image and audio are deliberately absent: their vocabularies
    /// are regions and spans, so a `GeneralizeDate` or a `Clamp` has
    /// no counterpart there. A policy covering those media names
    /// their operators explicitly.
    #[must_use]
    pub fn textual(spec: TextRedaction) -> Self {
        Self {
            text: Some(spec.clone()),
            tabular: Some(TabularRedaction::Cell { spec }),
            ..Self::default()
        }
    }

    /// See [`Self::text`]. Same shortcut, tabular slot.
    #[must_use]
    pub fn tabular(spec: TabularRedaction) -> Self {
        Self {
            tabular: Some(spec),
            ..Self::default()
        }
    }

    /// See [`Self::text`]. Same shortcut, image slot.
    #[must_use]
    pub fn image(spec: ImageRedaction) -> Self {
        Self {
            image: Some(spec),
            ..Self::default()
        }
    }

    /// See [`Self::text`]. Same shortcut, audio slot.
    #[must_use]
    pub fn audio(spec: AudioRedaction) -> Self {
        Self {
            audio: Some(spec),
            ..Self::default()
        }
    }

    /// `true` when no operator is set for any modality. A rule whose
    /// `redact` field is empty after deserialisation is an author
    /// error. The request validator rejects it.
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
    use super::{ModalityRedactions, TabularRedaction, TextRedaction};

    #[test]
    fn textual_reaches_cells_as_well_as_text() {
        // A rule built with `text` alone matches a tabular entity
        // and attaches nothing, so the cell survives unredacted
        // with no error. `textual` is what closes that.
        let action = ModalityRedactions::textual(TextRedaction::Erase);

        assert_eq!(action.text, Some(TextRedaction::Erase));
        assert_eq!(
            action.tabular,
            Some(TabularRedaction::Cell {
                spec: TextRedaction::Erase,
            }),
            "the same operator, applied to the cell's own text",
        );
    }

    #[test]
    fn textual_leaves_image_and_audio_alone() {
        // Deliberate: their vocabularies are regions and spans, so
        // an operator like `GeneralizeDate` has no counterpart. A
        // policy covering those media names their operators itself
        // rather than inheriting a substitute that would change
        // what the policy promises.
        let action = ModalityRedactions::textual(TextRedaction::Pseudonymize);

        assert!(action.image.is_none());
        assert!(action.audio.is_none());
    }
}
