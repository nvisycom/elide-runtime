//! [`PresetManifest`]: JSON-defined NLP-engine preset.
//!
//! A manifest is **pure data** — it describes a downloadable
//! HuggingFace token-classification model plus everything needed to
//! wire it into our pipeline: the repo + revision pinning, the model
//! and tokenizer file paths within the repo, optional content hashes,
//! the `id2label` vector, the base-label → entity mapping, and which
//! languages it supports.
//!
//! This module deliberately contains no I/O beyond reading the JSON
//! file itself, no engine construction, and no download logic.
//! Operators ship manifests as JSON files and reference them by path
//! from [`NlpPreset::Manifest`].
//!
//! Sample manifest for `dslim/bert-base-NER` lives at
//! `crates/nvisy-nlp/presets/dslim-bert-base-NER.json`.
//!
//! [`NlpPreset::Manifest`]: super::NlpPreset::Manifest

use std::collections::HashMap;
use std::path::Path;

use nvisy_ontology::entity::{EntityCategory, EntityKind};
use nvisy_ontology::primitive::LanguageTag;
use serde::{Deserialize, Serialize};

use crate::error::{Error, Result};

/// JSON-defined preset describing a HuggingFace token-classification
/// model.
///
/// Fields mirror what `OrtNerConfig` needs at construction plus the
/// HF coordinates used by the download path.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PresetManifest {
    /// Identifier surfaced through `RecognitionMethod::ner` on every
    /// produced entity. Usually the HF repo id.
    pub model_name: String,
    /// HuggingFace repo id (e.g. `"dslim/bert-base-NER"`).
    pub repo_id: String,
    /// Commit SHA the artifacts are pinned to. Required: presets must
    /// be content-addressed so the same manifest always loads the
    /// same bytes.
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
    /// Ordered label vector matching the model's argmax indices, as
    /// shipped in the HF `config.json` `id2label` field.
    pub id_to_label: Vec<String>,
    /// Map from BIO base label (prefix stripped, e.g. `"PER"` for
    /// `"B-PER"`/`"I-PER"`) to the entity it represents. Bases that
    /// don't appear in this map are dropped during recognition.
    pub label_map: HashMap<String, LabelMapEntry>,
    /// Languages the model was trained on. An empty list is treated
    /// as "any language" by the backend.
    #[serde(default)]
    pub supported_languages: Vec<String>,
    /// Maximum sequence length the model accepts. Defaults to 512
    /// (BERT-base typical) when absent.
    #[serde(default = "default_max_sequence_length")]
    pub max_sequence_length: usize,
}

/// `(EntityCategory, EntityKind)` pair as serialised in a manifest's
/// `label_map`.
///
/// JSON shape: `{"category": "personal_identity", "kind": "person_name"}`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct LabelMapEntry {
    /// Broad bucket the entity belongs to.
    pub category: EntityCategory,
    /// Specific entity kind.
    pub kind: EntityKind,
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

    /// Check that every base label in `label_map` has at least one
    /// corresponding BIO entry in `id_to_label`. Catches typos like
    /// `"PERSON"` in the map when the model emits `"B-PER"`.
    pub(super) fn validate_label_map(&self) -> Result<()> {
        let bases: std::collections::HashSet<&str> = self
            .id_to_label
            .iter()
            .map(|label| split_bio_base(label))
            .collect();
        let unknown: Vec<&str> = self
            .label_map
            .keys()
            .filter(|k| !bases.contains(k.as_str()))
            .map(String::as_str)
            .collect();
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
    use std::path::PathBuf;

    use super::*;

    #[test]
    fn parses_minimal_manifest() {
        let json = r#"{
            "model_name": "dslim/bert-base-NER",
            "repo_id": "dslim/bert-base-NER",
            "revision": "d1a3e8f13f8c3566299d95fcfc9a8d2382a9affc",
            "model_file": "onnx/model.onnx",
            "tokenizer_file": "onnx/tokenizer.json",
            "id_to_label": ["O", "B-PER", "I-PER"],
            "label_map": {
                "PER": {"category": "personal_identity", "kind": "person_name"}
            },
            "supported_languages": ["en"]
        }"#;
        let manifest = PresetManifest::from_slice(json.as_bytes()).unwrap();
        assert_eq!(manifest.model_name, "dslim/bert-base-NER");
        assert_eq!(manifest.max_sequence_length, 512); // default
        assert_eq!(manifest.id_to_label.len(), 3);
        assert_eq!(manifest.label_map.len(), 1);
        let langs = manifest.parsed_languages();
        assert_eq!(langs.len(), 1);
    }

    #[test]
    fn parses_label_map_to_entity_pair() {
        let json = r#"{
            "model_name": "x",
            "repo_id": "x/y",
            "revision": "abc",
            "model_file": "m.onnx",
            "tokenizer_file": "t.json",
            "id_to_label": ["O"],
            "label_map": {
                "PER": {"category": "personal_identity", "kind": "person_name"},
                "ORG": {"category": "organizational", "kind": "organization_name"}
            }
        }"#;
        let manifest = PresetManifest::from_slice(json.as_bytes()).unwrap();
        let per = manifest.label_map.get("PER").unwrap();
        assert_eq!(per.category, EntityCategory::PersonalIdentity);
        assert_eq!(per.kind, EntityKind::PersonName);
        assert_eq!(
            manifest.label_map.get("ORG").unwrap().kind,
            EntityKind::OrganizationName,
        );
        // PathBuf round-trip: confirms model_file is an owned String,
        // not a borrow into the input slice that would prevent passing
        // `manifest` to other modules.
        let _ = PathBuf::from(&manifest.model_file);
    }

    #[test]
    fn rejects_invalid_category() {
        let json = r#"{
            "model_name": "x",
            "repo_id": "x/y",
            "revision": "abc",
            "model_file": "m.onnx",
            "tokenizer_file": "t.json",
            "id_to_label": ["O"],
            "label_map": {
                "PER": {"category": "nonsense_category", "kind": "person_name"}
            }
        }"#;
        let err = PresetManifest::from_slice(json.as_bytes()).unwrap_err();
        assert!(matches!(err, Error::Backend(_)));
    }

    #[test]
    fn validate_label_map_passes_when_bases_match() {
        let json = r#"{
            "model_name": "x", "repo_id": "x/y", "revision": "abc",
            "model_file": "m.onnx", "tokenizer_file": "t.json",
            "id_to_label": ["O", "B-PER", "I-PER", "B-ORG", "I-ORG"],
            "label_map": {
                "PER": {"category": "personal_identity", "kind": "person_name"},
                "ORG": {"category": "organizational", "kind": "organization_name"}
            }
        }"#;
        let manifest = PresetManifest::from_slice(json.as_bytes()).unwrap();
        manifest.validate_label_map().unwrap();
    }

    #[test]
    fn validate_label_map_rejects_orphan_base() {
        let json = r#"{
            "model_name": "x", "repo_id": "x/y", "revision": "abc",
            "model_file": "m.onnx", "tokenizer_file": "t.json",
            "id_to_label": ["O", "B-PER", "I-PER"],
            "label_map": {
                "PERSON": {"category": "personal_identity", "kind": "person_name"}
            }
        }"#;
        let manifest = PresetManifest::from_slice(json.as_bytes()).unwrap();
        let err = manifest.validate_label_map().unwrap_err();
        assert!(
            err.to_string().contains("PERSON"),
            "error should name the orphan label: {err}",
        );
    }
}
