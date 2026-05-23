//! [`OrtBackend`] — runs a HuggingFace token-classification model
//! exported to ONNX via the [`ort`] crate.
//!
//! The backend is split into two layers so the heavy ML path can be
//! mocked in unit tests:
//!
//! - `Inferencer` (crate-private): tokenize-output-to-logits. The
//!   production impl wraps an `ort::Session`; tests inject a
//!   closure-based mock.
//! - [`OrtBackend`]: orchestration — tokenize input, dispatch to
//!   the inferencer, argmax + softmax over logits, fold BIO tags
//!   into spans, build [`Entity`] values.
//!
//! [`ort`]: https://crates.io/crates/ort
//! [`Entity`]: nvisy_ontology::entity::Entity

use std::collections::HashMap;
use std::fmt;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use ndarray::Array2;
use nvisy_ontology::entity::{
    Entities, Entity, EntityCategory, EntityKind, Location, ModelKind, RecognitionMethod,
    TextLocation,
};
use nvisy_ontology::primitive::{Confidence, LanguageTag};
use ort::session::Session;
use ort::value::Value;
use tokenizers::{Encoding, Tokenizer};

use super::NerBackend;
use crate::error::{Error, Result};

const LABEL_OUTSIDE: &str = "O";

/// Configuration for [`OrtBackend`].
///
/// The user provides paths to an ONNX model and its tokenizer, the
/// model's full ordered label vector (the one HuggingFace stores as
/// `id2label` in `config.json`), and a map from base label
/// (BIO prefix stripped) to [`EntityKind`].
///
/// Use [`id_to_label_from_config_json`] to parse the standard HF
/// `config.json` next to a downloaded model.
///
/// [`id_to_label_from_config_json`]: id_to_label_from_config_json
#[derive(Debug, Clone)]
pub struct OrtParams {
    /// Path to the `.onnx` model file.
    pub model_path: PathBuf,
    /// Path to the matching `tokenizer.json`.
    pub tokenizer_path: PathBuf,
    /// Ordered label vector matching the model's argmax indices, as
    /// shipped in the HF `config.json` `id2label` field. Index 0 is
    /// commonly the "outside" tag (`"O"`) but the position is whatever
    /// the model exports.
    pub id_to_label: Vec<String>,
    /// Map from base label (BIO prefix stripped, e.g. `"PER"` for
    /// `"B-PER"`/`"I-PER"`) to the entity it represents. Bases that
    /// don't appear in this map cause detections to be dropped — use
    /// this to filter the model's noisier labels (e.g. CoNLL `MISC`).
    ///
    /// Must not contain the literal `"O"` key — it's reserved as the
    /// "outside" tag and is rejected at construction time.
    pub label_map: HashMap<String, (EntityCategory, EntityKind)>,
    /// Maximum sequence length the model accepts. Inputs longer than
    /// this are truncated by the tokenizer before inference.
    pub max_sequence_length: usize,
    /// Model identifier surfaced through
    /// [`RecognitionMethod::ner`] on every produced entity. Useful
    /// for provenance tracking.
    pub model_name: String,
}

/// Parse the standard HuggingFace `config.json` `id2label` field into
/// an ordered label vector suitable for [`OrtParams::id_to_label`].
///
/// Expects the `id2label` keys to be integer strings (`"0"`, `"1"`, …)
/// densely covering `0..num_labels`. Returns an error if the field is
/// missing, malformed, or has gaps.
pub fn id_to_label_from_config_json(path: impl AsRef<Path>) -> Result<Vec<String>> {
    let path = path.as_ref();
    let bytes = std::fs::read(path)
        .map_err(|e| Error::Backend(format!("config.json read {}: {e}", path.display())))?;
    let value: serde_json::Value = serde_json::from_slice(&bytes)
        .map_err(|e| Error::Backend(format!("config.json parse {}: {e}", path.display())))?;
    let map = value
        .get("id2label")
        .and_then(|v| v.as_object())
        .ok_or_else(|| {
            Error::Backend(format!("config.json missing id2label: {}", path.display()))
        })?;

    let mut entries: Vec<(usize, String)> = map
        .iter()
        .map(|(k, v)| {
            let idx: usize = k
                .parse()
                .map_err(|_| Error::Backend(format!("id2label key '{k}' is not an integer")))?;
            let label = v
                .as_str()
                .ok_or_else(|| Error::Backend(format!("id2label[{k}] is not a string")))?
                .to_owned();
            Ok((idx, label))
        })
        .collect::<Result<Vec<_>>>()?;
    entries.sort_by_key(|(idx, _)| *idx);

    for (expected, (actual, _)) in entries.iter().enumerate() {
        if expected != *actual {
            return Err(Error::Backend(format!(
                "id2label has gap at index {expected} (found {actual})",
            )));
        }
    }
    Ok(entries.into_iter().map(|(_, label)| label).collect())
}

/// Inference interface that produces flat per-token logits.
///
/// Returned shape is `(seq_len * num_labels)` — flat row-major.
/// `num_labels` is returned alongside so callers can iterate rows
/// via `chunks_exact(num_labels)`.
///
/// Lives behind a trait so unit tests can inject canned logits
/// without loading an ONNX model. Crate-private: external NER
/// backends should implement [`NerBackend`] directly.
pub(crate) trait Inferencer: Send + Sync {
    /// Run inference and return per-token label logits.
    ///
    /// Returns `(logits, num_labels)` where `logits.len() ==
    /// encoding.get_ids().len() * num_labels`.
    fn infer(&self, encoding: &Encoding) -> Result<(Vec<f32>, usize)>;
}

/// ONNX-Runtime-backed inferencer.
pub(crate) struct OrtInferencer {
    session: Mutex<Session>,
    model_path: PathBuf,
}

impl OrtInferencer {
    /// Load a model from disk.
    pub(crate) fn from_file(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref().to_owned();
        let session = Session::builder()
            .map_err(|e| Error::ModelLoad {
                path: path.clone(),
                cause: e.to_string(),
            })?
            .commit_from_file(&path)
            .map_err(|e| Error::ModelLoad {
                path: path.clone(),
                cause: e.to_string(),
            })?;
        Ok(Self {
            session: Mutex::new(session),
            model_path: path,
        })
    }
}

impl Inferencer for OrtInferencer {
    fn infer(&self, encoding: &Encoding) -> Result<(Vec<f32>, usize)> {
        let ids: Vec<i64> = encoding.get_ids().iter().map(|&x| x as i64).collect();
        let attention: Vec<i64> = encoding
            .get_attention_mask()
            .iter()
            .map(|&x| x as i64)
            .collect();
        let seq_len = ids.len();

        let input_ids = Array2::from_shape_vec((1, seq_len), ids)
            .map_err(|e| Error::Inference(format!("input_ids shape: {e}")))?;
        let attention_mask = Array2::from_shape_vec((1, seq_len), attention)
            .map_err(|e| Error::Inference(format!("attention_mask shape: {e}")))?;
        // Standard BERT-family exports declare `token_type_ids` as a
        // required input even when single-sequence inference makes the
        // values irrelevant. A zero tensor is the canonical fill.
        let token_type_ids = Array2::<i64>::zeros((1, seq_len));

        let session = self
            .session
            .lock()
            .map_err(|_| Error::Inference("ORT session mutex poisoned".to_owned()))?;

        let input_ids_v = Value::from_array(input_ids)
            .map_err(|e| Error::Inference(format!("input_ids value: {e}")))?;
        let attention_v = Value::from_array(attention_mask)
            .map_err(|e| Error::Inference(format!("attention_mask value: {e}")))?;
        let token_type_v = Value::from_array(token_type_ids)
            .map_err(|e| Error::Inference(format!("token_type_ids value: {e}")))?;

        // `ort::inputs!` returns a `Result` on rc.9, hence the `?`.
        let inputs = ort::inputs![
            "input_ids" => input_ids_v,
            "attention_mask" => attention_v,
            "token_type_ids" => token_type_v,
        ]
        .map_err(|e| Error::Inference(format!("inputs build: {e}")))?;
        let outputs = session
            .run(inputs)
            .map_err(|e| Error::Inference(e.to_string()))?;

        // rc.9's `try_extract_tensor` returns an `ArrayView<T, IxDyn>`,
        // unlike rc.12 which returns `(shape, data)` directly.
        let array = outputs[0]
            .try_extract_tensor::<f32>()
            .map_err(|e| Error::Inference(format!("logits extract: {e}")))?;
        let shape = array.shape();

        if shape.len() != 3 || shape[0] != 1 || shape[1] != seq_len {
            return Err(Error::Inference(format!(
                "unexpected logits shape {shape:?}; expected [1, {seq_len}, num_labels]",
            )));
        }
        let num_labels = shape[2];
        let data: Vec<f32> = array.iter().copied().collect();
        Ok((data, num_labels))
    }
}

impl fmt::Debug for OrtInferencer {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("OrtInferencer")
            .field("model_path", &self.model_path)
            .finish_non_exhaustive()
    }
}

/// A [`NerBackend`] that runs a HuggingFace token-classification model
/// exported to ONNX.
pub struct OrtBackend {
    state: Arc<OrtState>,
}

struct OrtState {
    tokenizer: Tokenizer,
    inferencer: Box<dyn Inferencer>,
    id_to_label: Vec<String>,
    label_map: HashMap<String, (EntityCategory, EntityKind)>,
    model_name: String,
    supported_languages: Vec<LanguageTag>,
}

impl OrtBackend {
    /// Construct from a [`OrtParams`], loading the model and
    /// tokenizer from disk.
    pub fn new(config: OrtParams) -> Result<Self> {
        let inferencer = Box::new(OrtInferencer::from_file(&config.model_path)?);
        Self::with_inferencer(config, inferencer)
    }

    /// Construct with an arbitrary [`Inferencer`]. Used by tests to
    /// inject canned logits without loading a real model.
    pub(crate) fn with_inferencer(
        config: OrtParams,
        inferencer: Box<dyn Inferencer>,
    ) -> Result<Self> {
        if config.label_map.contains_key(LABEL_OUTSIDE) {
            return Err(Error::Backend(format!(
                "OrtParams.label_map must not contain the reserved '{LABEL_OUTSIDE}' label",
            )));
        }
        if config.id_to_label.is_empty() {
            return Err(Error::Backend(
                "OrtParams.id_to_label must not be empty".to_owned(),
            ));
        }

        let mut tokenizer = Tokenizer::from_file(&config.tokenizer_path)
            .map_err(|e| Error::Tokenizer(format!("{}: {e}", config.tokenizer_path.display())))?;

        // Wire truncation so OrtParams.max_sequence_length is
        // actually respected. Encode/decode round-tripping is
        // unaffected because we only consult offsets relative to the
        // (truncated) encoding.
        let truncation = tokenizers::TruncationParams {
            max_length: config.max_sequence_length,
            ..Default::default()
        };
        tokenizer
            .with_truncation(Some(truncation))
            .map_err(|e| Error::Tokenizer(format!("set truncation: {e}")))?;

        let state = OrtState {
            tokenizer,
            inferencer,
            id_to_label: config.id_to_label,
            label_map: config.label_map,
            model_name: config.model_name,
            supported_languages: Vec::new(),
        };
        Ok(Self {
            state: Arc::new(state),
        })
    }

    /// Set the languages this backend was trained on. The default is
    /// an empty list, treated as "any language" by
    /// [`NerBackend::recognize`] — a non-empty list causes the
    /// `language` hint to be validated and an
    /// [`Error::UnsupportedLanguage`] returned on mismatch.
    pub fn with_supported_languages(mut self, languages: Vec<LanguageTag>) -> Self {
        Arc::get_mut(&mut self.state)
            .expect("with_supported_languages must be called before any clone")
            .supported_languages = languages;
        self
    }

    fn recognize_blocking(&self, text: &str) -> Result<Entities> {
        let encoding = self
            .state
            .tokenizer
            .encode(text, true)
            .map_err(|e| Error::Tokenizer(e.to_string()))?;
        let (logits, num_labels) = self.state.inferencer.infer(&encoding)?;
        let predictions = argmax_softmax(&logits, num_labels);
        Ok(fold_predictions(
            encoding.get_offsets(),
            encoding.get_special_tokens_mask(),
            &predictions,
            &self.state.id_to_label,
            &self.state.label_map,
            &self.state.model_name,
        ))
    }
}

/// Fold per-token BIO predictions into entity spans. Pure function so
/// the BIO-continuation logic can be unit-tested without loading a
/// tokenizer or a model.
///
/// `offsets[i]` and `special_mask[i]` come from a HuggingFace
/// `Encoding`, but the function doesn't care — anything with matching
/// per-token slices works.
fn fold_predictions(
    offsets: &[(usize, usize)],
    special_mask: &[u32],
    predictions: &[Prediction],
    id_to_label: &[String],
    label_map: &HashMap<String, (EntityCategory, EntityKind)>,
    model_name: &str,
) -> Entities {
    let mut entities: Vec<Entity> = Vec::new();
    let mut current: Option<CurrentSpan> = None;

    for (i, pred) in predictions.iter().enumerate() {
        let is_special = special_mask.get(i).copied().unwrap_or(0) == 1;
        let (start_char, end_char) = offsets.get(i).copied().unwrap_or((0, 0));
        if is_special || start_char == end_char {
            flush(&mut current, &mut entities, label_map, model_name);
            continue;
        }

        let label = id_to_label.get(pred.label_id).map(String::as_str);
        let (prefix, base) = split_bio(label.unwrap_or(LABEL_OUTSIDE));

        if base == LABEL_OUTSIDE {
            flush(&mut current, &mut entities, label_map, model_name);
            continue;
        }

        match (prefix, &current) {
            ("B", _) | (_, None) => {
                flush(&mut current, &mut entities, label_map, model_name);
                current = Some(CurrentSpan {
                    base: base.to_owned(),
                    start: start_char,
                    end: end_char,
                    confidence_sum: pred.confidence,
                    token_count: 1,
                });
            }
            (_, Some(c)) if c.base == base => {
                let c = current.as_mut().expect("matched Some above");
                c.end = end_char;
                c.confidence_sum += pred.confidence;
                c.token_count += 1;
            }
            _ => {
                flush(&mut current, &mut entities, label_map, model_name);
                current = Some(CurrentSpan {
                    base: base.to_owned(),
                    start: start_char,
                    end: end_char,
                    confidence_sum: pred.confidence,
                    token_count: 1,
                });
            }
        }
    }
    flush(&mut current, &mut entities, label_map, model_name);

    entities.into_iter().collect()
}

fn flush(
    current: &mut Option<CurrentSpan>,
    entities: &mut Vec<Entity>,
    label_map: &HashMap<String, (EntityCategory, EntityKind)>,
    model_name: &str,
) {
    if let Some(span) = current.take()
        && let Some(entity) = build_entity(&span, label_map, model_name)
    {
        entities.push(entity);
    }
}

fn build_entity(
    span: &CurrentSpan,
    label_map: &HashMap<String, (EntityCategory, EntityKind)>,
    model_name: &str,
) -> Option<Entity> {
    let (category, kind) = label_map.get(&span.base).copied()?;
    let location = TextLocation::builder()
        .with_start_offset(span.start)
        .with_end_offset(span.end)
        .build()
        .ok()?;
    let raw_confidence = if span.token_count == 0 {
        0.0
    } else {
        span.confidence_sum / span.token_count as f64
    };
    let confidence = Confidence::clamped(raw_confidence);
    Entity::builder()
        .with_category(category)
        .with_entity_kind(kind)
        .with_recognition_methods(vec![RecognitionMethod::ner(
            model_name,
            ModelKind::SelfHosted,
        )])
        .with_confidence(confidence)
        .with_location(Location::from(location))
        .build()
        .ok()
}

impl fmt::Debug for OrtBackend {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("OrtBackend")
            .field("model", &self.state.model_name)
            .field("labels", &self.state.id_to_label.len())
            .finish_non_exhaustive()
    }
}

#[async_trait]
impl NerBackend for OrtBackend {
    async fn recognize(
        &self,
        text: &str,
        language: Option<&LanguageTag>,
        // Ignored: this backend's label vector is baked into the
        // ONNX file. The `Engine` post-filters our output against
        // any caller-supplied allowlist.
        _requested_kinds: Option<&[EntityKind]>,
    ) -> Result<Entities> {
        // Validate language against `supported_languages` when set.
        if let Some(lang) = language
            && !self.state.supported_languages.is_empty()
            && !self.state.supported_languages.contains(lang)
        {
            return Err(Error::UnsupportedLanguage(lang.clone()));
        }

        // Dispatch the blocking inference onto a pool thread so we
        // don't starve the tokio executor on long sequences.
        let state = Arc::clone(&self.state);
        let text = text.to_owned();
        tokio::task::spawn_blocking(move || {
            let backend = OrtBackend { state };
            backend.recognize_blocking(&text)
        })
        .await
        .map_err(|e| Error::Inference(format!("join error: {e}")))?
    }
}

/// Per-token prediction: which label id won the argmax and at what
/// softmax probability.
#[derive(Debug, Clone, Copy)]
struct Prediction {
    label_id: usize,
    confidence: f64,
}

/// Internal span accumulator used while folding BIO tags.
struct CurrentSpan {
    base: String,
    start: usize,
    end: usize,
    confidence_sum: f64,
    token_count: usize,
}

/// Compute per-token argmax + softmax confidence on a flat
/// `(seq_len * num_labels)` logits buffer.
fn argmax_softmax(logits: &[f32], num_labels: usize) -> Vec<Prediction> {
    logits
        .chunks_exact(num_labels)
        .map(|row| {
            // Numerically stable softmax: subtract row max, exp, sum,
            // divide; pick the argmax index along the way.
            let mut max = f32::NEG_INFINITY;
            let mut max_idx = 0usize;
            for (i, &v) in row.iter().enumerate() {
                if v > max {
                    max = v;
                    max_idx = i;
                }
            }
            let mut sum = 0.0_f64;
            let mut argmax_exp = 0.0_f64;
            for (i, &v) in row.iter().enumerate() {
                let e = ((v - max) as f64).exp();
                sum += e;
                if i == max_idx {
                    argmax_exp = e;
                }
            }
            let confidence = if sum > 0.0 { argmax_exp / sum } else { 0.0 };
            Prediction {
                label_id: max_idx,
                confidence,
            }
        })
        .collect()
}

/// Split a BIO label into `(prefix, base)`. `"B-PER"` → `("B", "PER")`.
/// Unprefixed labels (`"PER"`) get `("", "PER")`; `"O"` returns
/// `("", "O")`.
fn split_bio(label: &str) -> (&str, &str) {
    if let Some((prefix, base)) = label.split_once('-') {
        (prefix, base)
    } else {
        ("", label)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn person_label_map() -> HashMap<String, (EntityCategory, EntityKind)> {
        let mut m = HashMap::new();
        m.insert(
            "PER".to_owned(),
            (EntityCategory::PersonalIdentity, EntityKind::PersonName),
        );
        m
    }

    fn three_labels() -> Vec<String> {
        vec!["O".to_owned(), "B-PER".to_owned(), "I-PER".to_owned()]
    }

    fn pred(label_id: usize, confidence: f64) -> Prediction {
        Prediction {
            label_id,
            confidence,
        }
    }

    /// Argmax winner per row plus row-wise softmax normalisation,
    /// without round-tripping through the tokenizer.
    #[test]
    fn argmax_softmax_picks_max_and_normalises() {
        let logits = vec![0.1, 0.7, 0.2, 0.9, 0.05, 0.05];
        let preds = argmax_softmax(&logits, 3);
        assert_eq!(preds.len(), 2);
        assert_eq!(preds[0].label_id, 1);
        assert_eq!(preds[1].label_id, 0);
        for p in &preds {
            assert!(
                p.confidence > 0.0 && p.confidence <= 1.0,
                "confidence {} not in (0,1]",
                p.confidence,
            );
        }
    }

    #[test]
    fn split_bio_strips_prefix() {
        assert_eq!(split_bio("B-PER"), ("B", "PER"));
        assert_eq!(split_bio("I-PER"), ("I", "PER"));
        assert_eq!(split_bio("O"), ("", "O"));
        assert_eq!(split_bio("PER"), ("", "PER"));
    }

    #[test]
    fn id_to_label_from_config_json_parses() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.json");
        std::fs::write(&path, r#"{"id2label":{"0":"O","1":"B-PER","2":"I-PER"}}"#).unwrap();
        let labels = id_to_label_from_config_json(&path).unwrap();
        assert_eq!(labels, vec!["O", "B-PER", "I-PER"]);
    }

    #[test]
    fn id_to_label_from_config_json_rejects_gap() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.json");
        std::fs::write(&path, r#"{"id2label":{"0":"O","2":"I-PER"}}"#).unwrap();
        let err = id_to_label_from_config_json(&path).unwrap_err();
        assert!(matches!(err, Error::Backend(_)));
    }

    /// Two separate `B-PER` spans separated by `O` tokens fold into
    /// two distinct entities with the right per-token offsets.
    #[test]
    fn fold_predictions_produces_two_spans() {
        // 6 "tokens": [CLS] + 4 words + [SEP].
        // Offsets are simulated; only the relative layout matters.
        let offsets = vec![(0, 0), (0, 5), (6, 11), (12, 14), (15, 19), (0, 0)];
        let special_mask = vec![1, 0, 0, 0, 0, 1];
        let predictions = vec![
            pred(0, 1.0),  // [CLS]: O
            pred(1, 0.95), // Alice: B-PER
            pred(0, 1.0),  // works: O
            pred(0, 1.0),  // at: O
            pred(1, 0.95), // Acme: B-PER
            pred(0, 1.0),  // [SEP]: O
        ];
        let entities = fold_predictions(
            &offsets,
            &special_mask,
            &predictions,
            &three_labels(),
            &person_label_map(),
            "test",
        );
        assert_eq!(entities.len(), 2);
        for e in &entities {
            assert_eq!(e.entity_kind, EntityKind::PersonName);
        }
        let first = entities.iter().next().unwrap();
        let loc = first.location.as_text().expect("text location");
        assert_eq!((loc.start_offset, loc.end_offset), (0, 5)); // "Alice"
    }

    /// `B-PER` opens a span and an immediately-following `I-PER`
    /// (same base) extends it instead of starting a new one. Then `O`
    /// flushes and the next `B-PER` opens a fresh span. This is the
    /// real BIO-continuation rule and where bugs hide.
    #[test]
    fn fold_predictions_continues_inside_span_with_i_tag() {
        let offsets = vec![(0, 0), (0, 5), (6, 11), (12, 14), (15, 19), (0, 0)];
        let special_mask = vec![1, 0, 0, 0, 0, 1];
        let predictions = vec![
            pred(0, 1.0),  // [CLS]: O
            pred(1, 0.95), // B-PER (start)
            pred(2, 0.95), // I-PER (continue)
            pred(0, 1.0),  // O (flush)
            pred(1, 0.95), // B-PER (new span)
            pred(0, 1.0),  // [SEP]: O
        ];
        let entities = fold_predictions(
            &offsets,
            &special_mask,
            &predictions,
            &three_labels(),
            &person_label_map(),
            "test",
        );
        assert_eq!(entities.len(), 2);
        let first = entities.iter().next().unwrap();
        let loc = first.location.as_text().expect("text location");
        assert_eq!((loc.start_offset, loc.end_offset), (0, 11)); // "Alice works"
    }
}
