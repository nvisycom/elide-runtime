//! Enrichers: the components that produce context for recognizers.
//!
//! One module per backend, each holding its deployment
//! configuration. `elide-provider` turns these lineups into the
//! enrichers elide runs.
//!
//! Enrichers run before recognition and stamp side-channel data —
//! a language hint, OCR'd text layout, audio transcript segments —
//! onto the per-request context, so a recognizer downstream reads
//! what they found.
//!
//! Language detection sits here too, as [`Lingua`] rather than a
//! lineup: it is always wired, because a recognizer that filters
//! by language needs it, so there is nothing to select between —
//! only a candidate set to narrow.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::Component;
use super::backend::{OcrBackend, SttBackend};
use crate::feature::Lingua;

/// The deployment's enricher lineups, one per backend kind.
///
/// An enricher attaches to the analyzer for its own modality: OCR
/// to image, STT to audio. At most one per lineup today; more is a
/// [`Configuration`](elide::ErrorKind::Configuration) error at
/// request compile.
#[derive(Debug, Clone, Default)]
#[derive(Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", default)]
pub struct Enrichers {
    /// The OCR lineup, attaching to the image analyzer.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub ocr: Vec<Component<OcrBackend>>,
    /// The STT lineup, attaching to the audio analyzer.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub stt: Vec<Component<SttBackend>>,
    /// Language detection, which is always wired.
    ///
    /// Not a lineup: there is one detector, so this narrows what it
    /// may return rather than choosing between backends.
    #[serde(skip_serializing_if = "Lingua::is_unrestricted")]
    pub lingua: Lingua,
}
