//! [`NerDetection`]: NER-specific knobs in the plan detection
//! config.
//!
//! Cross-recognizer hints (`entity_kinds`) live directly on
//! [`Detection`] because every recognizer honors them. This struct
//! exists for any future NER-specific knobs and to carry the
//! enable/disable toggle so operators can opt the recognizer in or
//! out independently.
//!
//! Backend selection lives on [`NerBackend`] in `nvisy-ner` and is
//! re-exported through [`crate::detection::NerBackend`] so config
//! authors see one canonical type.
//!
//! [`Detection`]: crate::detection::Detection

use nvisy_ner::NerBackend;
use serde::{Deserialize, Serialize};

/// NER-specific detection settings.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NerDetection {
    /// Enable this recognizer. When `false`, the recognizer is
    /// neither built nor dispatched, but the config is preserved
    /// so operators can toggle without losing it. Defaults to
    /// `true`.
    #[serde(default = "default_true")]
    pub enabled: bool,

    /// Backend selection for the NER pipeline. Defaults to
    /// [`NerBackend::Noop`] so a baseline runtime works without
    /// any inference service configured; switch to
    /// [`NerBackend::Bento`] to call the externalised
    /// `inference-gliner` service.
    ///
    /// [`NerBackend::Noop`]: nvisy_ner::NerBackend::Noop
    /// [`NerBackend::Bento`]: nvisy_ner::NerBackend::Bento
    #[serde(default)]
    pub backend: NerBackend,
}

impl Default for NerDetection {
    fn default() -> Self {
        Self {
            enabled: true,
            backend: NerBackend::default(),
        }
    }
}

fn default_true() -> bool {
    true
}
