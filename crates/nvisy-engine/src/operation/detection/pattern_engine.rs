//! [`PatternEngineRef`]: thin wrapper that holds either a borrowed reference
//! to the global [`PatternEngine`] singleton or an owned engine built
//! from custom config, exposed uniformly via [`Deref`].
//!
//! [`PatternEngine`]: nvisy_pattern::PatternEngine

use std::ops::Deref;

use nvisy_ontology::workflow::PatternDetection;

/// Holds either a borrowed reference to the global singleton or an
/// owned engine built from custom config.
pub(super) enum PatternEngineRef {
    Shared(&'static nvisy_pattern::PatternEngine),
    Owned(nvisy_pattern::PatternEngine),
}

impl PatternEngineRef {
    /// Resolve a [`PatternDetection`] config into either a borrowed
    /// reference to the shared singleton (when the config is the
    /// default empty shape) or a freshly built owned engine (when the
    /// config names patterns or sets a confidence threshold).
    pub(super) fn new(cfg: &PatternDetection) -> Self {
        let needs_custom = !cfg.patterns.is_empty() || cfg.confidence_threshold.is_some();
        if !needs_custom {
            return Self::Shared(nvisy_pattern::PatternEngine::instance());
        }
        let mut builder = nvisy_pattern::PatternEngine::builder();
        if !cfg.patterns.is_empty() {
            let names: Vec<&str> = cfg.patterns.iter().map(String::as_str).collect();
            builder = builder.with_patterns(&names);
        }
        if let Some(threshold) = cfg.confidence_threshold {
            builder = builder.with_confidence_threshold(threshold);
        }
        Self::Owned(builder.build().expect("pattern engine must compile"))
    }
}

impl Deref for PatternEngineRef {
    type Target = nvisy_pattern::PatternEngine;

    fn deref(&self) -> &Self::Target {
        match self {
            Self::Shared(e) => e,
            Self::Owned(e) => e,
        }
    }
}
