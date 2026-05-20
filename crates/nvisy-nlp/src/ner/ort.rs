//! [`OrtNerBackend`] — runs a HuggingFace token-classification model
//! exported to ONNX via the [`ort`] crate.
//!
//! The backend is split into two layers so the heavy ML path can be
//! mocked in unit tests:
//!
//! - [`Inferencer`]: tokenize-output-to-logits. The production impl
//!   wraps an `ort::Session`; tests inject a closure-based mock.
//! - [`OrtNerBackend`]: orchestration — tokenize input, dispatch to
//!   the inferencer, argmax over logits, fold BIO tags into spans,
//!   build [`Entity`] values.
//!
//! See `DESIGN.md` for the rationale.

use std::collections::HashMap;
use std::fmt;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use async_trait::async_trait;
use ndarray::Array2;
use nvisy_ontology::entity::{
    Entities, Entity, EntityCategory, EntityKind, Location, ModelKind, RecognitionMethod,
    TextLocation,
};
use nvisy_ontology::primitive::LanguageTag;
use ort::session::Session;
use ort::value::Value;
use tokenizers::{Encoding, Tokenizer};

use super::NerBackend;
use crate::error::NlpError;

/// Configuration for [`OrtNerBackend`].
///
/// The user provides paths to an ONNX model and its tokenizer, plus a
/// map from the model's emitted labels to [`EntityKind`]s. Labels are
/// expected to follow BIO conventions (`B-PER`, `I-PER`, `O`, etc.);
/// the prefix is stripped before lookup.
#[derive(Debug, Clone)]
pub struct OrtNerConfig {
    /// Path to the `.onnx` model file.
    pub model_path: PathBuf,
    /// Path to the matching `tokenizer.json`.
    pub tokenizer_path: PathBuf,
    /// Map from base label (BIO prefix stripped, uppercased) to
    /// [`EntityKind`]. Labels that don't appear in this map cause
    /// detections to be dropped.
    pub label_map: HashMap<String, (EntityCategory, EntityKind)>,
    /// Maximum sequence length the model accepts. Inputs longer than
    /// this are truncated.
    pub max_sequence_length: usize,
    /// Model identifier surfaced through
    /// [`RecognitionMethod::ner`] on every produced entity. Useful
    /// for provenance tracking.
    pub model_name: String,
}

/// Inference interface that produces per-token logits.
///
/// Returned shape is `[seq_len, num_labels]` — the batch dimension is
/// implicit (always 1 in this backend).
///
/// Lives behind a trait so unit tests can inject canned logits
/// without loading an ONNX model.
pub trait Inferencer: Send + Sync {
    /// Run inference and return per-token label logits.
    fn infer(&self, encoding: &Encoding) -> Result<Vec<Vec<f32>>, NlpError>;
}

/// ONNX-Runtime-backed inferencer.
///
/// Loads the model once at construction; subsequent [`infer`] calls
/// reuse the session.
///
/// [`infer`]: Inferencer::infer
pub struct OrtInferencer {
    session: Mutex<Session>,
    model_path: PathBuf,
}

impl OrtInferencer {
    /// Load a model from disk.
    pub fn from_file(path: impl AsRef<Path>) -> Result<Self, NlpError> {
        let path = path.as_ref().to_owned();
        let session = Session::builder()
            .map_err(|e| NlpError::ModelLoad {
                path: path.clone(),
                cause: e.to_string(),
            })?
            .commit_from_file(&path)
            .map_err(|e| NlpError::ModelLoad {
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
    fn infer(&self, encoding: &Encoding) -> Result<Vec<Vec<f32>>, NlpError> {
        let ids: Vec<i64> = encoding.get_ids().iter().map(|&x| x as i64).collect();
        let attention: Vec<i64> = encoding
            .get_attention_mask()
            .iter()
            .map(|&x| x as i64)
            .collect();
        let seq_len = ids.len();

        let input_ids = Array2::from_shape_vec((1, seq_len), ids)
            .map_err(|e| NlpError::Inference(format!("input_ids shape: {e}")))?;
        let attention_mask = Array2::from_shape_vec((1, seq_len), attention)
            .map_err(|e| NlpError::Inference(format!("attention_mask shape: {e}")))?;

        let mut session = self
            .session
            .lock()
            .map_err(|_| NlpError::Inference("ORT session mutex poisoned".to_owned()))?;

        let input_ids_v = Value::from_array(input_ids)
            .map_err(|e| NlpError::Inference(format!("input_ids value: {e}")))?;
        let attention_v = Value::from_array(attention_mask)
            .map_err(|e| NlpError::Inference(format!("attention_mask value: {e}")))?;

        let outputs = session
            .run(ort::inputs![
                "input_ids" => input_ids_v,
                "attention_mask" => attention_v,
            ])
            .map_err(|e| NlpError::Inference(e.to_string()))?;

        let (shape, data) = outputs[0]
            .try_extract_tensor::<f32>()
            .map_err(|e| NlpError::Inference(format!("logits extract: {e}")))?;

        if shape.len() != 3 || shape[0] != 1 || shape[1] as usize != seq_len {
            return Err(NlpError::Inference(format!(
                "unexpected logits shape {shape:?}; expected [1, {seq_len}, num_labels]",
            )));
        }
        let num_labels = shape[2] as usize;

        let mut rows: Vec<Vec<f32>> = Vec::with_capacity(seq_len);
        for token_idx in 0..seq_len {
            let start = token_idx * num_labels;
            rows.push(data[start..start + num_labels].to_vec());
        }
        Ok(rows)
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
pub struct OrtNerBackend {
    tokenizer: Tokenizer,
    inferencer: Box<dyn Inferencer>,
    id_to_label: Vec<String>,
    config: OrtNerConfig,
}

impl OrtNerBackend {
    /// Construct from a [`OrtNerConfig`], loading the model and
    /// tokenizer from disk.
    pub fn new(config: OrtNerConfig) -> Result<Self, NlpError> {
        let inferencer = Box::new(OrtInferencer::from_file(&config.model_path)?);
        Self::with_inferencer(config, inferencer)
    }

    /// Construct with an arbitrary [`Inferencer`]. Used by tests to
    /// inject canned logits.
    pub fn with_inferencer(
        config: OrtNerConfig,
        inferencer: Box<dyn Inferencer>,
    ) -> Result<Self, NlpError> {
        let tokenizer = Tokenizer::from_file(&config.tokenizer_path)
            .map_err(|e| NlpError::Tokenizer(format!("{}: {e}", config.tokenizer_path.display())))?;
        let id_to_label = label_order(&config.label_map);
        Ok(Self {
            tokenizer,
            inferencer,
            id_to_label,
            config,
        })
    }

    /// Run the backend without going through `&dyn NerBackend`. Used
    /// by tests for synchronous assertions.
    pub(crate) fn recognize_sync(&self, text: &str) -> Result<Entities, NlpError> {
        let encoding = self
            .tokenizer
            .encode(text, true)
            .map_err(|e| NlpError::Tokenizer(e.to_string()))?;
        let logits = self.inferencer.infer(&encoding)?;
        let predictions = argmax(&logits);
        Ok(self.fold_predictions(text, &encoding, &predictions))
    }

    fn fold_predictions(
        &self,
        text: &str,
        encoding: &Encoding,
        predictions: &[usize],
    ) -> Entities {
        let offsets = encoding.get_offsets();
        let special = encoding.get_special_tokens_mask();
        let mut entities: Vec<Entity> = Vec::new();
        let mut current: Option<CurrentSpan> = None;

        for (i, &label_id) in predictions.iter().enumerate() {
            let is_special = special.get(i).copied().unwrap_or(0) == 1;
            let (start_char, end_char) = offsets.get(i).copied().unwrap_or((0, 0));
            if is_special || start_char == end_char {
                flush(&mut current, &mut entities, text, self);
                continue;
            }

            let label = self.id_to_label.get(label_id).cloned();
            let (prefix, base) = split_bio(label.as_deref().unwrap_or("O"));

            if base == "O" {
                flush(&mut current, &mut entities, text, self);
                continue;
            }

            match (prefix, &current) {
                ("B", _) | (_, None) => {
                    flush(&mut current, &mut entities, text, self);
                    current = Some(CurrentSpan {
                        base: base.to_owned(),
                        start: start_char,
                        end: end_char,
                    });
                }
                (_, Some(c)) if c.base == base => {
                    let c = current.as_mut().expect("matched Some above");
                    c.end = end_char;
                }
                _ => {
                    flush(&mut current, &mut entities, text, self);
                    current = Some(CurrentSpan {
                        base: base.to_owned(),
                        start: start_char,
                        end: end_char,
                    });
                }
            }
        }
        flush(&mut current, &mut entities, text, self);

        entities.into_iter().collect()
    }

    fn build_entity(&self, _text: &str, span: &CurrentSpan) -> Option<Entity> {
        let (category, kind) = self.config.label_map.get(&span.base).copied()?;
        let location = TextLocation::builder()
            .with_start_offset(span.start)
            .with_end_offset(span.end)
            .build()
            .ok()?;
        Entity::builder()
            .with_category(category)
            .with_entity_kind(kind)
            .with_recognition_methods(vec![RecognitionMethod::ner(
                &self.config.model_name,
                ModelKind::SelfHosted,
            )])
            .with_confidence(0.85)
            .with_location(Location::from(location))
            .build()
            .ok()
    }
}

impl fmt::Debug for OrtNerBackend {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("OrtNerBackend")
            .field("model", &self.config.model_name)
            .field("labels", &self.id_to_label.len())
            .finish_non_exhaustive()
    }
}

#[async_trait]
impl NerBackend for OrtNerBackend {
    async fn recognize(
        &self,
        text: &str,
        _language: Option<&LanguageTag>,
    ) -> Result<Entities, NlpError> {
        self.recognize_sync(text)
    }
}

/// Internal span accumulator used while folding BIO tags.
struct CurrentSpan {
    base: String,
    start: usize,
    end: usize,
}

/// Push `current` (if any) onto `entities`, then clear it.
fn flush(
    current: &mut Option<CurrentSpan>,
    entities: &mut Vec<Entity>,
    text: &str,
    backend: &OrtNerBackend,
) {
    if let Some(span) = current.take()
        && let Some(entity) = backend.build_entity(text, &span)
    {
        entities.push(entity);
    }
}

/// Compute the per-position argmax of a `[seq_len, num_labels]` matrix.
fn argmax(logits: &[Vec<f32>]) -> Vec<usize> {
    logits
        .iter()
        .map(|row| {
            row.iter()
                .enumerate()
                .max_by(|a, b| a.1.partial_cmp(b.1).unwrap_or(std::cmp::Ordering::Equal))
                .map(|(i, _)| i)
                .unwrap_or(0)
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

/// Deterministic vector of label names for argmax indexing.
///
/// In v1 we don't read the label IDs from the ONNX model graph
/// (Presidio doesn't either). The user-supplied [`OrtNerConfig::label_map`]
/// keys are sorted and treated as IDs `0..N`. Real models export a
/// `config.json` with the `id2label` map — wiring that is deferred until
/// it's needed (it's purely a convenience over manually specifying the
/// label order at construction time).
fn label_order(map: &HashMap<String, (EntityCategory, EntityKind)>) -> Vec<String> {
    let mut labels: Vec<String> = std::iter::once("O".to_owned())
        .chain(map.keys().cloned())
        .collect();
    // Stable sort, "O" first so it can be label ID 0.
    labels.sort_by(|a, b| {
        if a == "O" {
            std::cmp::Ordering::Less
        } else if b == "O" {
            std::cmp::Ordering::Greater
        } else {
            a.cmp(b)
        }
    });
    labels.dedup();
    labels
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A canned inferencer that returns pre-defined logits per call.
    ///
    /// Used by future integration tests that have a `tokenizer.json`
    /// fixture; with one in place, instantiate `OrtNerBackend` via
    /// [`OrtNerBackend::with_inferencer`] passing this and exercise
    /// `recognize_sync` end-to-end without an ONNX model file.
    #[allow(dead_code)]
    struct CannedInferencer {
        logits: Vec<Vec<f32>>,
    }

    #[allow(dead_code)]
    impl Inferencer for CannedInferencer {
        fn infer(&self, _encoding: &Encoding) -> Result<Vec<Vec<f32>>, NlpError> {
            Ok(self.logits.clone())
        }
    }

    fn person_label_map() -> HashMap<String, (EntityCategory, EntityKind)> {
        let mut m = HashMap::new();
        m.insert(
            "PER".to_owned(),
            (EntityCategory::PersonalIdentity, EntityKind::PersonName),
        );
        m
    }

    #[test]
    fn label_order_puts_o_first() {
        let labels = label_order(&person_label_map());
        assert_eq!(labels[0], "O");
    }

    #[test]
    fn argmax_picks_max_per_row() {
        let logits = vec![vec![0.1, 0.7, 0.2], vec![0.9, 0.05, 0.05]];
        assert_eq!(argmax(&logits), vec![1, 0]);
    }

    #[test]
    fn split_bio_strips_prefix() {
        assert_eq!(split_bio("B-PER"), ("B", "PER"));
        assert_eq!(split_bio("I-PER"), ("I", "PER"));
        assert_eq!(split_bio("O"), ("", "O"));
        assert_eq!(split_bio("PER"), ("", "PER"));
    }
}
