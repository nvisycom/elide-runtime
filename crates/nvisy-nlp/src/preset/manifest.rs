//! [`PresetManifest`]: JSON-defined NLP-engine preset.
//!
//! A manifest is **pure data** — it describes a downloadable
//! HuggingFace model plus everything needed to wire it into our
//! pipeline. The shared coordinates (`model_name`, `repo_id`,
//! `revision`, artifact paths and hashes, supported languages) live
//! on the outer struct; everything backend-specific lives in the
//! `backend` field, a tagged-union enum where each variant carries
//! only the fields its decoding path actually consumes.
//!
//! Two backends ship today:
//!
//! - [`BackendConfig::OnnxBert`]: a BIO-tagged token-classification
//!   model (e.g. `dslim/bert-base-NER`) loaded via [`OrtBackend`].
//!   Carries `id_to_label`, a BIO base → entity `label_map`, and a
//!   `max_sequence_length`.
//! - [`BackendConfig::GlinerSpan`] / [`BackendConfig::GlinerToken`]:
//!   a zero-shot [GLiNER](https://github.com/urchade/GLiNER) model
//!   loaded via [`GlinerBackend`]. Carries only a raw-label →
//!   entity `label_map`; the two variants differ only in which
//!   decoding pipeline runs at inference time.
//!
//! This module deliberately contains no I/O beyond reading the JSON
//! file itself, no engine construction, and no download logic.
//! Operators ship manifests as JSON files and reference them by path
//! from [`NlpPreset::Manifest`].
//!
//! Reference manifests live at `crates/nvisy-nlp/presets/`.
//!
//! [`NlpPreset::Manifest`]: super::NlpPreset::Manifest
//! [`OrtBackend`]: crate::ner::OrtBackend
//! [`GlinerBackend`]: crate::ner::GlinerBackend

use std::path::Path;

use nvisy_ontology::primitive::LanguageTag;
use serde::{Deserialize, Serialize};

use crate::error::{Error, Result};
use crate::ner::LabelMap;

/// JSON-defined preset describing a downloadable HuggingFace model.
///
/// The variant-independent coordinates (HF repo, revision, artifact
/// paths, hashes, languages) live here; everything backend-specific
/// lives in [`backend`]. Operators ship one of these as a JSON file
/// and reference it by path from [`NlpPreset::Manifest`].
///
/// [`backend`]: Self::backend
/// [`NlpPreset::Manifest`]: super::NlpPreset::Manifest
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PresetManifest {
    /// Identifier surfaced through `RecognitionMethod::nlp_ner` on every
    /// produced entity. Usually the HF repo id.
    pub model_name: String,
    /// HuggingFace repo id (e.g. `"dslim/bert-base-NER"`).
    pub repo_id: String,
    /// Commit SHA the artifacts are pinned to. Required: presets
    /// must be content-addressed so the same manifest always loads
    /// the same bytes.
    pub revision: String,
    /// Path within the repo to the ONNX model file (e.g.
    /// `"onnx/model.onnx"`).
    pub model_file: String,
    /// Optional SHA-256 of the model file, hex-encoded (64 chars).
    /// When present, downloaded or supplied model bytes are verified
    /// against this hash and a mismatch fails the load. Strongly
    /// recommended for manifests that auto-download; harmless for
    /// explicit-path configurations.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model_sha256: Option<String>,
    /// Path within the repo to the tokenizer file (e.g.
    /// `"onnx/tokenizer.json"`).
    pub tokenizer_file: String,
    /// Optional SHA-256 of the tokenizer file, hex-encoded. Same
    /// semantics as `model_sha256`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tokenizer_sha256: Option<String>,
    /// Languages the model was trained on. An empty list is treated
    /// as "any language" by the backend.
    #[serde(default)]
    pub supported_languages: Vec<String>,
    /// Backend-specific configuration. The JSON tag is `"kind"`.
    pub backend: BackendConfig,
}

/// Backend-specific configuration. Each variant carries only the
/// fields its decoding path actually consumes.
///
/// Serialised as a `"kind"`-tagged JSON object, e.g.
/// `"backend": {"kind": "onnx-bert", "id_to_label": [...], ...}`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum BackendConfig {
    /// BERT-family token-classification model loaded via
    /// [`OrtBackend`]. Requires the `onnx` feature.
    ///
    /// [`OrtBackend`]: crate::ner::OrtBackend
    OnnxBert {
        /// Maximum sequence length the model accepts. Defaults to
        /// 512 (BERT-base typical).
        #[serde(default = "default_max_sequence_length")]
        max_sequence_length: usize,
        /// Ordered label vector matching the model's argmax indices,
        /// as shipped in the HF `config.json` `id2label` field.
        id_to_label: Vec<String>,
        /// Map from BIO base label (prefix stripped, e.g. `"PER"`
        /// for `"B-PER"`/`"I-PER"`) to the entity it represents.
        /// Bases that don't appear in this map are dropped during
        /// recognition.
        label_map: LabelMap,
    },

    /// GLiNER zero-shot model decoded via the span pipeline (e.g.
    /// `onnx-community/gliner_small-v2.1`). Requires the `gliner`
    /// feature.
    GlinerSpan {
        /// Map from GLiNER label string (e.g. `"person"`) to the
        /// entity it represents.
        label_map: LabelMap,
    },

    /// GLiNER zero-shot model decoded via the token pipeline (e.g.
    /// `onnx-community/gliner-multitask-large-v0.5`). Requires the
    /// `gliner` feature.
    GlinerToken {
        /// Map from GLiNER label string to the entity it represents.
        label_map: LabelMap,
    },
}

fn default_max_sequence_length() -> usize {
    512
}

impl PresetManifest {
    /// Read and parse a manifest from a JSON file on disk.
    pub fn from_file(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref();
        let bytes = std::fs::read(path)
            .map_err(|e| Error::Backend(format!("read manifest {}: {e}", path.display())))?;
        serde_json::from_slice(&bytes)
            .map_err(|e| Error::Backend(format!("parse manifest {}: {e}", path.display())))
    }

    /// Parse a manifest from a JSON byte slice. Useful when the
    /// manifest is embedded rather than read from disk.
    pub fn from_slice(bytes: &[u8]) -> Result<Self> {
        serde_json::from_slice(bytes).map_err(|e| Error::Backend(format!("parse manifest: {e}")))
    }

    /// Parse `supported_languages` into [`LanguageTag`]s, dropping
    /// any that fail to parse (with a warning trace).
    pub fn parsed_languages(&self) -> Vec<LanguageTag> {
        self.supported_languages
            .iter()
            .filter_map(|raw| match raw.parse() {
                Ok(tag) => Some(tag),
                Err(e) => {
                    tracing::warn!(
                        target: "nvisy_nlp::preset",
                        raw = %raw,
                        error = %e,
                        "manifest supported_language failed to parse; ignoring"
                    );
                    None
                }
            })
            .collect()
    }

    /// For [`BackendConfig::OnnxBert`], check that every base label
    /// in `label_map` has at least one corresponding BIO entry in
    /// `id_to_label`. Catches typos like `"PERSON"` in the map when
    /// the model emits `"B-PER"`. No-op for GLiNER variants, whose
    /// label space is open by construction.
    pub(super) fn validate_label_map(&self) -> Result<()> {
        let BackendConfig::OnnxBert {
            id_to_label,
            label_map,
            ..
        } = &self.backend
        else {
            return Ok(());
        };
        let bases: std::collections::HashSet<&str> =
            id_to_label.iter().map(|l| split_bio_base(l)).collect();
        let unknown: Vec<&str> = label_map.labels().filter(|k| !bases.contains(k)).collect();
        if unknown.is_empty() {
            Ok(())
        } else {
            Err(Error::Backend(format!(
                "preset manifest label_map references base labels not present in id_to_label: {unknown:?}",
            )))
        }
    }
}

/// Strip a BIO prefix (`"B-PER"` → `"PER"`). Bare labels (`"O"`,
/// `"PER"`) pass through unchanged. Mirrors `ner::ort::split_bio` but
/// returns just the base.
fn split_bio_base(label: &str) -> &str {
    label.split_once('-').map(|(_, base)| base).unwrap_or(label)
}

#[cfg(test)]
mod tests {
    use nvisy_ontology::entity::{EntityCategory, EntityKind};

    use super::*;

    /// Build an OnnxBert manifest with the given `id_to_label` and
    /// `label_map`, leaving everything else minimal. Used to exercise
    /// `validate_label_map`.
    fn onnx_bert(id_to_label: &[&str], label_map: &[(&str, EntityKind)]) -> PresetManifest {
        let mut lm = LabelMap::new();
        for (k, kind) in label_map {
            lm.insert(*k, kind.category(), *kind);
        }
        PresetManifest {
            model_name: "x".into(),
            repo_id: "x/y".into(),
            revision: "abc".into(),
            model_file: "m.onnx".into(),
            model_sha256: None,
            tokenizer_file: "t.json".into(),
            tokenizer_sha256: None,
            supported_languages: vec![],
            backend: BackendConfig::OnnxBert {
                id_to_label: id_to_label.iter().map(|s| (*s).to_owned()).collect(),
                label_map: lm,
                max_sequence_length: 64,
            },
        }
    }

    #[test]
    fn validate_label_map_passes_when_bases_match() {
        let manifest = onnx_bert(
            &["O", "B-PER", "I-PER", "B-ORG", "I-ORG"],
            &[
                ("PER", EntityKind::PersonName),
                ("ORG", EntityKind::OrganizationName),
            ],
        );
        manifest.validate_label_map().unwrap();
    }

    #[test]
    fn validate_label_map_rejects_orphan_base() {
        let manifest = onnx_bert(
            &["O", "B-PER", "I-PER"],
            &[("PERSON", EntityKind::PersonName)],
        );
        let err = manifest.validate_label_map().unwrap_err();
        assert!(
            err.to_string().contains("PERSON"),
            "error should name the orphan label: {err}",
        );
    }

    #[test]
    fn validate_label_map_is_noop_for_gliner() {
        // GLiNER accepts arbitrary labels: even a nonsense key like
        // this should pass validation.
        let mut lm = LabelMap::new();
        lm.insert(
            "literally anything",
            EntityCategory::PersonalIdentity,
            EntityKind::PersonName,
        );
        let manifest = PresetManifest {
            model_name: "x".into(),
            repo_id: "x/y".into(),
            revision: "abc".into(),
            model_file: "m.onnx".into(),
            model_sha256: None,
            tokenizer_file: "t.json".into(),
            tokenizer_sha256: None,
            supported_languages: vec![],
            backend: BackendConfig::GlinerSpan { label_map: lm },
        };
        manifest.validate_label_map().unwrap();
    }
}
