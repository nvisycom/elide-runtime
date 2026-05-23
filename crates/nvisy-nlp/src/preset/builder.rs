//! Compose a [`PresetManifest`] + resolved artifact paths into an
//! [`Engine`].
//!
//! Pure composition: this module assumes the artifacts have already
//! been downloaded (or supplied) and verified. It validates the
//! manifest's `label_map` against `id_to_label` first so config typos
//! surface immediately at construction time rather than as silently
//! dropped entities at recognition time.

use std::path::PathBuf;
use std::sync::Arc;

use super::manifest::PresetManifest;
use crate::Engine;
use crate::error::{Error, Result};
use crate::language::LinguaLanguagePolicy;
use crate::ner::{OrtNerBackend, OrtNerConfig};

/// Build an [`Engine`] from `manifest` plus resolved artifact paths.
///
/// # Errors
///
/// - Manifest label-map validation failure (typo in base label).
/// - ORT backend construction failure (model load, tokenizer load).
/// - Engine builder failure (missing dependencies — should not happen
///   here because both `ner` and `language_policy` are supplied).
pub(super) fn build_engine(
    manifest: PresetManifest,
    model_path: PathBuf,
    tokenizer_path: PathBuf,
) -> Result<Arc<Engine>> {
    manifest.validate_label_map()?;
    let languages = manifest.parsed_languages();
    let cfg = into_ort_config(manifest, model_path, tokenizer_path);
    let mut backend = OrtNerBackend::new(cfg)?;
    if !languages.is_empty() {
        backend = backend.with_supported_languages(languages);
    }
    Engine::builder()
        .with_ner_backend(backend)
        .with_language_policy(LinguaLanguagePolicy)
        .build()
        .map(Arc::new)
        .map_err(|e| Error::Backend(e.to_string()))
}

/// Lower a manifest into the concrete config the ORT backend expects.
fn into_ort_config(
    manifest: PresetManifest,
    model_path: PathBuf,
    tokenizer_path: PathBuf,
) -> OrtNerConfig {
    OrtNerConfig {
        model_path,
        tokenizer_path,
        id_to_label: manifest.id_to_label,
        label_map: manifest
            .label_map
            .into_iter()
            .map(|(k, v)| (k, (v.category, v.kind)))
            .collect(),
        max_sequence_length: manifest.max_sequence_length,
        model_name: manifest.model_name,
    }
}
