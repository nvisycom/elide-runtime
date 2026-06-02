//! [`PatternDetection`]: pattern-recognizer settings.
//!
//! Today the only knob is the enable/disable toggle — the registry
//! composition (which shipped patterns + dictionaries ship, which
//! extras to load) isn't yet plan-configurable.

use std::sync::Arc;

use nvisy_core::{EntityRecognizer, Result};
use nvisy_ontology::modality::Text;
use nvisy_pattern::recognition::{PatternRecognizer, PatternRegistry};
use nvisy_pattern::shipped;
use serde::{Deserialize, Serialize};

/// Pattern-recognizer settings.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PatternDetection {
    /// Enable this recognizer. When `false`, the recognizer is
    /// neither built nor dispatched, but the config is preserved
    /// so operators can toggle without losing it. Defaults to
    /// `true` — pattern detection is always-on out of the box.
    #[serde(default = "default_true")]
    pub enabled: bool,
}

impl PatternDetection {
    /// Build the engine-side pattern recognizer.
    ///
    /// Today the config doesn't carry registry customization — the
    /// recognizer is built from the shipped registry
    /// (`shipped::patterns::all()` + `shipped::dictionaries::all()`)
    /// regardless. The `enabled` field is honoured at the
    /// registration site in
    /// [`RecognizerRegistry::from_config`],
    /// not here.
    ///
    /// # Errors
    ///
    /// Returns an error if the underlying recognizer fails to
    /// compile (would itself be a bug — every shipped pattern is
    /// asserted to compile by `nvisy-pattern`'s own unit tests).
    ///
    /// [`RecognizerRegistry::from_config`]: crate::detection::RecognizerRegistry::from_config
    pub fn build(&self) -> Result<Arc<dyn EntityRecognizer<Text>>> {
        let recognizer = PatternRecognizer::builder()
            .with_registry(default_registry())
            .build()?;
        Ok(Arc::new(recognizer))
    }
}

impl Default for PatternDetection {
    fn default() -> Self {
        Self { enabled: true }
    }
}

/// The default registry: every shipped pattern + every shipped
/// dictionary.
fn default_registry() -> PatternRegistry {
    let mut registry = PatternRegistry::new();
    for p in shipped::patterns::all() {
        registry = registry.with_pattern(p);
    }
    for d in shipped::dictionaries::all() {
        registry = registry.with_dictionary(d);
    }
    registry
}

fn default_true() -> bool {
    true
}
