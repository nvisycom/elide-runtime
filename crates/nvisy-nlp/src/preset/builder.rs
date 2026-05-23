//! Compose a [`PresetManifest`] + resolved artifact paths into an
//! [`Engine`].
//!
//! Pure composition: this module assumes the artifacts have already
//! been downloaded (or supplied) and verified. It validates the
//! manifest's label map first (for the BERT backend) so config typos
//! surface immediately at construction time rather than as silently
//! dropped entities at recognition time, then dispatches to the
//! matching backend feature.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use nvisy_ontology::primitive::LanguageTag;

use super::manifest::{BackendConfig, LabelMapEntry, PresetManifest};
use crate::Engine;
use crate::error::{Error, Result};
#[cfg(any(feature = "onnx", feature = "gliner"))]
use crate::language::LinguaLanguagePolicy;

/// Build an [`Engine`] from `manifest` plus resolved artifact paths.
///
/// Dispatches on the manifest's [`BackendConfig`] to the matching
/// backend crate feature. Returns [`Error::Backend`] when the
/// manifest targets a backend whose feature isn't enabled in this
/// build, so operators get a clear "enable feature X" message rather
/// than a silent path.
pub(super) fn build_engine(
    manifest: PresetManifest,
    model_path: PathBuf,
    tokenizer_path: PathBuf,
) -> Result<Arc<Engine>> {
    manifest.validate_label_map()?;
    let languages = manifest.parsed_languages();
    match manifest.backend {
        BackendConfig::OnnxBert {
            id_to_label,
            label_map,
            max_sequence_length,
        } => build_onnx_bert(
            manifest.model_name,
            languages,
            id_to_label,
            label_map,
            max_sequence_length,
            model_path,
            tokenizer_path,
        ),
        BackendConfig::GlinerSpan { label_map } => build_gliner(
            manifest.model_name,
            languages,
            label_map,
            GlinerMode::Span,
            model_path,
            tokenizer_path,
        ),
        BackendConfig::GlinerToken { label_map } => build_gliner(
            manifest.model_name,
            languages,
            label_map,
            GlinerMode::Token,
            model_path,
            tokenizer_path,
        ),
    }
}

// `GlinerMode` is re-imported here under cfg so the same identifier
// flows through the BackendConfig → build_gliner dispatch under the
// `gliner` feature and shows up as a stub type otherwise. The stub
// variant is what the disabled-feature path passes around.
#[cfg(feature = "gliner")]
use crate::ner::GlinerMode;
#[cfg(not(feature = "gliner"))]
enum GlinerMode {
    Span,
    Token,
}

#[cfg(feature = "onnx")]
fn build_onnx_bert(
    model_name: String,
    languages: Vec<LanguageTag>,
    id_to_label: Vec<String>,
    label_map: HashMap<String, LabelMapEntry>,
    max_sequence_length: usize,
    model_path: PathBuf,
    tokenizer_path: PathBuf,
) -> Result<Arc<Engine>> {
    use crate::ner::{OrtNerBackend, OrtNerConfig};

    let cfg = OrtNerConfig {
        model_path,
        tokenizer_path,
        id_to_label,
        label_map: label_map
            .into_iter()
            .map(|(k, v)| (k, (v.category, v.kind)))
            .collect(),
        max_sequence_length,
        model_name,
    };
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

#[cfg(not(feature = "onnx"))]
fn build_onnx_bert(
    _model_name: String,
    _languages: Vec<LanguageTag>,
    _id_to_label: Vec<String>,
    _label_map: HashMap<String, LabelMapEntry>,
    _max_sequence_length: usize,
    _model_path: PathBuf,
    _tokenizer_path: PathBuf,
) -> Result<Arc<Engine>> {
    Err(Error::Backend(
        "preset manifest targets the `onnx-bert` backend but `nvisy-nlp` \
         was built without the `onnx` feature"
            .to_owned(),
    ))
}

#[cfg(feature = "gliner")]
fn build_gliner(
    model_name: String,
    languages: Vec<LanguageTag>,
    label_map: HashMap<String, LabelMapEntry>,
    mode: GlinerMode,
    model_path: PathBuf,
    tokenizer_path: PathBuf,
) -> Result<Arc<Engine>> {
    use crate::ner::{GlinerBackend, GlinerConfig};

    let cfg = GlinerConfig {
        model_path,
        tokenizer_path,
        mode,
        label_map: label_map
            .into_iter()
            .map(|(k, v)| (k, (v.category, v.kind)))
            .collect(),
        model_name,
    };
    let mut backend = GlinerBackend::new(cfg)?;
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

#[cfg(not(feature = "gliner"))]
fn build_gliner(
    _model_name: String,
    _languages: Vec<LanguageTag>,
    _label_map: HashMap<String, LabelMapEntry>,
    _mode: GlinerMode,
    _model_path: PathBuf,
    _tokenizer_path: PathBuf,
) -> Result<Arc<Engine>> {
    Err(Error::Backend(
        "preset manifest targets a GLiNER backend but `nvisy-nlp` was \
         built without the `gliner` feature"
            .to_owned(),
    ))
}
