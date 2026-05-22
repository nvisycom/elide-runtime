//! [`NerEngine`]: prebuilt NLP-engine presets selectable from config.
//!
//! Today there is one variant — [`NerEngine::Default`] — backed by
//! [`NoopNerBackend`]. The intent of this enum is to grow into a
//! curated list of bundled engines (CoNLL-03 English, multilingual
//! Presidio-style, etc.) as real model bundles land. Config-driven
//! recognizer assembly references a `NerEngine` variant by name and
//! gets back a fully-constructed `Arc<Engine>`.
//!
//! [`NoopNerBackend`]: crate::ner::NoopNerBackend

use std::sync::Arc;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::Engine;
use crate::error::Result;
use crate::language::LinguaLanguagePolicy;
use crate::ner::NoopNerBackend;

/// Prebuilt NLP-engine preset.
///
/// Picked by a single name from the workflow config; the recognizer
/// crate calls [`build`] to materialise the corresponding
/// [`Engine`].
///
/// TODO: add real model bundles (e.g. CoNLL-03 English, multilingual
/// Presidio analogue) and downgrade `Default` to point at one of
/// them; drop the `test-utils` default feature in this crate's
/// `Cargo.toml` once that lands.
///
/// [`build`]: Self::build
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
#[derive(Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum NerEngine {
    /// No-op engine — returns no entities. Placeholder until a real
    /// prebuilt model bundle is shipped. Backed by [`NoopNerBackend`].
    ///
    /// [`NoopNerBackend`]: crate::ner::NoopNerBackend
    #[default]
    Default,
}

impl NerEngine {
    /// Construct the [`Engine`] this preset selects.
    pub fn build(self) -> Result<Arc<Engine>> {
        match self {
            Self::Default => Engine::builder()
                .with_ner_backend(NoopNerBackend)
                .with_language_policy(LinguaLanguagePolicy)
                .build()
                .map(Arc::new)
                .map_err(|e| crate::Error::Backend(e.to_string())),
        }
    }
}
