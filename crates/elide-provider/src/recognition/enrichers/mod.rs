//! Enrichers: the components that produce context for recognizers.
//!
//! One module per backend, each holding its deployment
//! configuration. [`compile`] turns those lineups into the
//! enrichers elide runs.
//!
//! Enrichers run before recognition and stamp side-channel data —
//! a language hint, OCR'd text layout, audio transcript segments —
//! onto the per-request context, so a recognizer downstream reads
//! what they found. Language detection has no configuration (elide
//! wires lingua unconditionally) so it appears only in [`compile`].

mod ocr;
mod stt;

pub(crate) mod compile;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::Component;

/// The deployment's enricher lineups, one per backend kind.
///
/// An enricher attaches to the analyzer for its own modality: OCR
/// to image, STT to audio. At most one per lineup today; more is a
/// [`Configuration`](elide::ErrorKind::Configuration) error at
/// request compile.
#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", default)]
pub struct Enrichers {
    /// The OCR lineup, attaching to the image analyzer.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub ocr: Vec<Component<OcrBackend>>,
    /// The STT lineup, attaching to the audio analyzer.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub stt: Vec<Component<SttBackend>>,
}

pub use self::ocr::OcrBackend;
pub use self::stt::SttBackend;
