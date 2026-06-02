//! Pattern-recognizer wiring.
//!
//! The engine registers a [`nvisy_pattern::recognition::PatternRecognizer`]
//! directly — it already implements
//! [`nvisy_core::Recognizer<Text>`](nvisy_core::Recognizer). No
//! engine-side adapter wrapper.
//!
//! [`PatternDetection`] is the operator-facing config; the
//! [`build_recognizer`] free function turns that config into the
//! built recognizer wrapped in an `Arc`. Pattern detection is
//! always-on by default: when the `[detection.pattern]` section is
//! omitted entirely, the engine still registers a pattern
//! recognizer with the shipped registry.

use std::sync::Arc;

use nvisy_core::{Recognizer, Result};
use nvisy_ontology::modality::Text;
use nvisy_pattern::recognition::{PatternRecognizer as InnerPatternRecognizer, PatternRegistry};
use nvisy_pattern::shipped;

use crate::pipeline::PatternDetection;

/// Build the engine-side pattern recognizer from a
/// [`PatternDetection`] config.
///
/// Today the config doesn't carry registry customization — the
/// recognizer is built from the shipped registry
/// (`shipped::patterns::all()` + `shipped::dictionaries::all()`)
/// regardless. The `enabled` field is honoured at the registration
/// site in
/// [`RecognizerRegistry::from_config`](super::RecognizerRegistry::from_config),
/// not here.
///
/// # Errors
///
/// Returns an error if the underlying recognizer fails to compile
/// (would itself be a bug — every shipped pattern is asserted to
/// compile by `nvisy-pattern`'s own unit tests).
pub fn build_recognizer(_cfg: &PatternDetection) -> Result<Arc<dyn Recognizer<Text>>> {
    let recognizer = InnerPatternRecognizer::builder()
        .with_registry(default_registry())
        .build()?;
    Ok(Arc::new(recognizer))
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
