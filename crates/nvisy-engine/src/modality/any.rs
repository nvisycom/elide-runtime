//! Type-erased [`Modality::Location`] enum.

use nvisy_codec::core::ModalityKind;
use nvisy_core::modality::{AudioLocation, ImageLocation, TabularLocation, TextLocation};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Type-erased [`Modality::Location`] carrying both the modality
/// tag and the typed coordinate value. Used by the redaction
/// override surface where the caller hasn't yet pinned a
/// `M: Modality` type at the API boundary.
///
/// Wire shape matches the [`ModalityKind`] tag plus a flattened
/// location object:
///
/// ```json
/// { "modality": "text", "start": 0, "end": 10 }
/// ```
///
/// [`Modality::Location`]: nvisy_core::modality::Modality::Location
#[derive(Debug, Clone, PartialEq)]
#[derive(Serialize, Deserialize, JsonSchema)]
#[serde(tag = "modality", rename_all = "snake_case")]
pub enum AnyLocation {
    Text(TextLocation),
    Tabular(TabularLocation),
    Image(ImageLocation),
    Audio(AudioLocation),
}

impl AnyLocation {
    /// The modality this location belongs to.
    #[must_use]
    pub fn kind(&self) -> ModalityKind {
        match self {
            Self::Text(_) => ModalityKind::Text,
            Self::Tabular(_) => ModalityKind::Tabular,
            Self::Image(_) => ModalityKind::Image,
            Self::Audio(_) => ModalityKind::Audio,
        }
    }
}
