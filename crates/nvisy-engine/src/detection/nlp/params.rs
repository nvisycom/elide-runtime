//! [`NlpDetection`]: NER-specific knobs in the workflow detection
//! config.
//!
//! Cross-recognizer hints (`entity_kinds`, `confidence_threshold`)
//! live directly on [`Detection`] because every recognizer honors
//! them. This struct exists for any future NER-specific knobs and
//! to carry the enable/disable toggle so operators can opt the
//! recognizer in or out independently.
//!
//! [`Detection`]: crate::detection::Detection

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// NER-specific detection settings.
#[derive(Debug, Clone, PartialEq, Eq)]
#[derive(Serialize, Deserialize, JsonSchema)]
pub struct NlpDetection {
    /// Enable this recognizer. When `false`, the recognizer is
    /// neither built nor dispatched, but the config is preserved
    /// so operators can toggle without losing it. Defaults to
    /// `true`.
    #[serde(default = "default_true")]
    pub enabled: bool,
}

impl Default for NlpDetection {
    fn default() -> Self {
        Self { enabled: true }
    }
}

fn default_true() -> bool {
    true
}
