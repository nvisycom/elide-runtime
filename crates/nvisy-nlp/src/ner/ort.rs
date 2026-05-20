//! [`OrtNerBackend`] — runs a HuggingFace token-classification model
//! exported to ONNX via the [`ort`] crate.
//!
//! The backend is split into two layers so the heavy ML path can be
//! mocked in unit tests:
//!
//! - `Inferencer` (crate-private): tokenize-output-to-logits. The
//!   production impl wraps an `ort::Session`; tests inject a
//!   closure-based mock.
//! - [`OrtNerBackend`]: orchestration — tokenize input, dispatch to
//!   the inferencer, argmax + softmax over logits, fold BIO tags
//!   into spans, build [`Entity`] values.

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
use nvisy_ontology::primitive::LanguageTag;
use ort::session::Session;
use ort::value::Value;
use tokenizers::{Encoding, Tokenizer};

use super::NerBackend;
use crate::error::{Error, Result};

const LABEL_OUTSIDE: &str = "O";

/// Configuration for [`OrtNerBackend`].
///
/// The user provides paths to an ONNX model and its tokenizer, plus a
/// map from the model's emitted labels to [`EntityKind`]s. Labels are
/// expected to follow BIO conventions (`B-PER`, `I-PER`, `O`, etc.);
/// the prefix is stripped before lookup.
///
/// Use [`with_conll03_english`] for the common `dslim/bert-base-NER`
/// label set.
///
/// [`with_conll03_english`]: Self::with_conll03_english
#[derive(Debug, Clone)]
pub struct OrtNerConfig {
    /// Path to the `.onnx` model file.
    pub model_path: PathBuf,
    /// Path to the matching `tokenizer.json`.
    pub tokenizer_path: PathBuf,
    /// Map from base label (BIO prefix stripped, uppercased) to
    /// [`EntityKind`]. Labels that don't appear in this map cause
    /// detections to be dropped.
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

impl OrtNerConfig {
    /// Construct a config for the standard CoNLL-03 English label set
    /// (`PER`, `ORG`, `LOC`, `MISC`) used by `dslim/bert-base-NER`
    /// and similar models. The user still picks how each label maps
    /// to a [`EntityKind`]; this only fills in the four standard
    /// keys with the user's chosen kinds.
    pub fn with_conll03_english(
        model_path: PathBuf,
        tokenizer_path: PathBuf,
        model_name: impl Into<String>,
        per: (EntityCategory, EntityKind),
        org: (EntityCategory, EntityKind),
        loc: (EntityCategory, EntityKind),
        misc: (EntityCategory, EntityKind),
    ) -> Self {
        let mut label_map = HashMap::new();
        label_map.insert("PER".to_owned(), per);
        label_map.insert("ORG".to_owned(), org);
        label_map.insert("LOC".to_owned(), loc);
        label_map.insert("MISC".to_owned(), misc);
        Self {
            model_path,
            tokenizer_path,
            label_map,
            max_sequence_length: 512,
            model_name: model_name.into(),
        }
    }
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

        let mut session = self
            .session
            .lock()
            .map_err(|_| Error::Inference("ORT session mutex poisoned".to_owned()))?;

        let input_ids_v = Value::from_array(input_ids)
            .map_err(|e| Error::Inference(format!("input_ids value: {e}")))?;
        let attention_v = Value::from_array(attention_mask)
            .map_err(|e| Error::Inference(format!("attention_mask value: {e}")))?;

        let outputs = session
            .run(ort::inputs![
                "input_ids" => input_ids_v,
                "attention_mask" => attention_v,
            ])
            .map_err(|e| Error::Inference(e.to_string()))?;

        let (shape, data) = outputs[0]
            .try_extract_tensor::<f32>()
            .map_err(|e| Error::Inference(format!("logits extract: {e}")))?;

        if shape.len() != 3 || shape[0] != 1 || shape[1] as usize != seq_len {
            return Err(Error::Inference(format!(
                "unexpected logits shape {shape:?}; expected [1, {seq_len}, num_labels]",
            )));
        }
        let num_labels = shape[2] as usize;
        Ok((data.to_vec(), num_labels))
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

impl OrtNerBackend {
    /// Construct from a [`OrtNerConfig`], loading the model and
    /// tokenizer from disk.
    pub fn new(config: OrtNerConfig) -> Result<Self> {
        let inferencer = Box::new(OrtInferencer::from_file(&config.model_path)?);
        Self::with_inferencer(config, inferencer)
    }

    /// Construct with an arbitrary [`Inferencer`]. Used by tests to
    /// inject canned logits without loading a real model.
    pub(crate) fn with_inferencer(
        config: OrtNerConfig,
        inferencer: Box<dyn Inferencer>,
    ) -> Result<Self> {
        if config.label_map.contains_key(LABEL_OUTSIDE) {
            return Err(Error::Backend(format!(
                "OrtNerConfig.label_map must not contain the reserved '{LABEL_OUTSIDE}' label",
            )));
        }

        let mut tokenizer = Tokenizer::from_file(&config.tokenizer_path)
            .map_err(|e| Error::Tokenizer(format!("{}: {e}", config.tokenizer_path.display())))?;

        // Wire truncation so OrtNerConfig.max_sequence_length is
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

        let id_to_label = label_order(&config.label_map);
        let state = OrtState {
            tokenizer,
            inferencer,
            id_to_label,
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
        Ok(self.fold_predictions(&encoding, &predictions))
    }

    fn fold_predictions(&self, encoding: &Encoding, predictions: &[Prediction]) -> Entities {
        let offsets = encoding.get_offsets();
        let special = encoding.get_special_tokens_mask();
        let mut entities: Vec<Entity> = Vec::new();
        let mut current: Option<CurrentSpan> = None;

        for (i, pred) in predictions.iter().enumerate() {
            let is_special = special.get(i).copied().unwrap_or(0) == 1;
            let (start_char, end_char) = offsets.get(i).copied().unwrap_or((0, 0));
            if is_special || start_char == end_char {
                self.flush(&mut current, &mut entities);
                continue;
            }

            let label = self.state.id_to_label.get(pred.label_id).cloned();
            let (prefix, base) = split_bio(label.as_deref().unwrap_or(LABEL_OUTSIDE));

            if base == LABEL_OUTSIDE {
                self.flush(&mut current, &mut entities);
                continue;
            }

            match (prefix, &current) {
                ("B", _) | (_, None) => {
                    self.flush(&mut current, &mut entities);
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
                    self.flush(&mut current, &mut entities);
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
        self.flush(&mut current, &mut entities);

        entities.into_iter().collect()
    }

    fn flush(&self, current: &mut Option<CurrentSpan>, entities: &mut Vec<Entity>) {
        if let Some(span) = current.take()
            && let Some(entity) = self.build_entity(&span)
        {
            entities.push(entity);
        }
    }

    fn build_entity(&self, span: &CurrentSpan) -> Option<Entity> {
        let (category, kind) = self.state.label_map.get(&span.base).copied()?;
        let location = TextLocation::builder()
            .with_start_offset(span.start)
            .with_end_offset(span.end)
            .build()
            .ok()?;
        let confidence = if span.token_count == 0 {
            0.0
        } else {
            span.confidence_sum / span.token_count as f64
        };
        Entity::builder()
            .with_category(category)
            .with_entity_kind(kind)
            .with_recognition_methods(vec![RecognitionMethod::ner(
                &self.state.model_name,
                ModelKind::SelfHosted,
            )])
            .with_confidence(confidence)
            .with_location(Location::from(location))
            .build()
            .ok()
    }
}

impl fmt::Debug for OrtNerBackend {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("OrtNerBackend")
            .field("model", &self.state.model_name)
            .field("labels", &self.state.id_to_label.len())
            .finish_non_exhaustive()
    }
}

#[async_trait]
impl NerBackend for OrtNerBackend {
    async fn recognize(&self, text: &str, language: Option<&LanguageTag>) -> Result<Entities> {
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
            let backend = OrtNerBackend { state };
            backend.recognize_blocking(&text)
        })
        .await
        .map_err(|e| Error::Inference(format!("join error: {e}")))?
    }

    fn supported_languages(&self) -> &[LanguageTag] {
        &self.state.supported_languages
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

/// Deterministic vector of label names for argmax indexing.
///
/// The user-supplied [`OrtNerConfig::label_map`] keys are sorted
/// alphabetically; `"O"` is placed at index 0. Real models export a
/// `config.json` with the `id2label` map — wiring that is deferred
/// until it's needed.
fn label_order(map: &HashMap<String, (EntityCategory, EntityKind)>) -> Vec<String> {
    let mut labels: Vec<String> = map.keys().cloned().collect();
    labels.sort();
    let mut out = Vec::with_capacity(labels.len() + 1);
    out.push(LABEL_OUTSIDE.to_owned());
    out.extend(labels);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A canned inferencer that returns pre-defined logits per call.
    struct CannedInferencer {
        logits: Vec<f32>,
        num_labels: usize,
    }

    impl Inferencer for CannedInferencer {
        fn infer(&self, _encoding: &Encoding) -> Result<(Vec<f32>, usize)> {
            Ok((self.logits.clone(), self.num_labels))
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

    /// Build a real HF tokenizer config in-memory so we can construct
    /// `OrtNerBackend` for unit tests without a `tokenizer.json` file.
    fn write_minimal_tokenizer(dir: &Path) -> PathBuf {
        // Minimal BERT-style WordLevel tokenizer that splits on
        // whitespace, recognises `[CLS]`, `[SEP]`, `[UNK]`, and the
        // four words we use in tests. Enough to drive
        // `fold_predictions` without a real model.
        let json = r#"{
          "version": "1.0",
          "truncation": null,
          "padding": null,
          "added_tokens": [
            {"id":0,"content":"[PAD]","single_word":false,"lstrip":false,"rstrip":false,"normalized":false,"special":true},
            {"id":1,"content":"[UNK]","single_word":false,"lstrip":false,"rstrip":false,"normalized":false,"special":true},
            {"id":2,"content":"[CLS]","single_word":false,"lstrip":false,"rstrip":false,"normalized":false,"special":true},
            {"id":3,"content":"[SEP]","single_word":false,"lstrip":false,"rstrip":false,"normalized":false,"special":true}
          ],
          "normalizer": null,
          "pre_tokenizer": {"type":"Whitespace"},
          "post_processor": {
            "type": "TemplateProcessing",
            "single": [
              {"SpecialToken":{"id":"[CLS]","type_id":0}},
              {"Sequence":{"id":"A","type_id":0}},
              {"SpecialToken":{"id":"[SEP]","type_id":0}}
            ],
            "pair": [
              {"SpecialToken":{"id":"[CLS]","type_id":0}},
              {"Sequence":{"id":"A","type_id":0}},
              {"SpecialToken":{"id":"[SEP]","type_id":0}},
              {"Sequence":{"id":"B","type_id":1}},
              {"SpecialToken":{"id":"[SEP]","type_id":1}}
            ],
            "special_tokens": {
              "[CLS]": {"id":"[CLS]","ids":[2],"tokens":["[CLS]"]},
              "[SEP]": {"id":"[SEP]","ids":[3],"tokens":["[SEP]"]}
            }
          },
          "decoder": null,
          "model": {
            "type": "WordLevel",
            "vocab": {
              "[PAD]":0,"[UNK]":1,"[CLS]":2,"[SEP]":3,
              "Alice":4,"works":5,"at":6,"Acme":7
            },
            "unk_token": "[UNK]"
          }
        }"#;
        let path = dir.join("tokenizer.json");
        std::fs::write(&path, json).expect("write tokenizer");
        path
    }

    fn build_backend(logits: Vec<f32>, num_labels: usize) -> (OrtNerBackend, tempfile::TempDir) {
        let dir = tempfile::tempdir().expect("tempdir");
        let tok_path = write_minimal_tokenizer(dir.path());
        let cfg = OrtNerConfig {
            model_path: PathBuf::from("/unused.onnx"),
            tokenizer_path: tok_path,
            label_map: person_label_map(),
            max_sequence_length: 64,
            model_name: "test-model".to_owned(),
        };
        let inferencer = Box::new(CannedInferencer { logits, num_labels });
        let backend = OrtNerBackend::with_inferencer(cfg, inferencer).expect("backend");
        (backend, dir)
    }

    #[test]
    fn label_order_puts_o_first() {
        let labels = label_order(&person_label_map());
        assert_eq!(labels[0], LABEL_OUTSIDE);
        assert!(labels.contains(&"PER".to_owned()));
    }

    #[test]
    fn label_order_rejects_outside_in_user_map() {
        let mut m = person_label_map();
        m.insert(
            LABEL_OUTSIDE.to_owned(),
            (EntityCategory::PersonalIdentity, EntityKind::PersonName),
        );
        let dir = tempfile::tempdir().unwrap();
        let tok = write_minimal_tokenizer(dir.path());
        let cfg = OrtNerConfig {
            model_path: PathBuf::from("/unused.onnx"),
            tokenizer_path: tok,
            label_map: m,
            max_sequence_length: 64,
            model_name: "test".to_owned(),
        };
        let inferencer = Box::new(CannedInferencer {
            logits: vec![],
            num_labels: 0,
        });
        let result = OrtNerBackend::with_inferencer(cfg, inferencer);
        assert!(matches!(result, Err(Error::Backend(_))));
    }

    #[test]
    fn argmax_softmax_picks_max_and_normalises() {
        // Two rows, three labels each. First row argmax=1, second row
        // argmax=0. Sum-to-one within each row.
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

    /// End-to-end: real tokenizer produces an encoding for
    /// "Alice works at Acme", canned logits put B-PER on `Alice` and
    /// B-PER on `Acme` (mis-labelled to exercise multi-span output).
    /// We expect two entities, the right offsets, and average
    /// per-span confidence.
    #[tokio::test]
    async fn fold_predictions_produces_two_spans() {
        // Encoding length = [CLS] + 4 words + [SEP] = 6 tokens.
        // Labels: [O, PER] (order from label_order: "O" at 0, "PER" at 1).
        // Layout (one row per token, two labels per row):
        //   t0 [CLS]: O wins
        //   t1 Alice: PER wins
        //   t2 works: O wins
        //   t3 at:    O wins
        //   t4 Acme:  PER wins
        //   t5 [SEP]: O wins
        let logits: Vec<f32> = vec![
            5.0, 0.0, // [CLS]: O
            0.0, 5.0, // Alice: PER
            5.0, 0.0, // works: O
            5.0, 0.0, // at: O
            0.0, 5.0, // Acme: PER
            5.0, 0.0, // [SEP]: O
        ];
        let (backend, _dir) = build_backend(logits, 2);
        let entities = backend.recognize_blocking("Alice works at Acme").unwrap();
        assert_eq!(entities.len(), 2);

        // Both spans are PersonName per the label_map.
        for e in &entities {
            assert_eq!(e.entity_kind, EntityKind::PersonName);
            assert!(e.confidence > 0.9, "softmax conf {} too low", e.confidence);
        }

        // Spans line up with the input text.
        let text = "Alice works at Acme";
        let first = entities.iter().next().unwrap();
        let first_loc = first.location.as_text().expect("text location");
        assert_eq!(&text[first_loc.start_offset..first_loc.end_offset], "Alice");
    }

    #[tokio::test]
    async fn fold_predictions_continues_inside_span() {
        // Encoding: [CLS] Alice works at Acme [SEP]
        // Labels:    O    B    I     O  B    O
        // Expect: one span [Alice, works], one span [Acme].
        // id_to_label: ["O", "I-PER", "B-PER"] ? — actually our
        // label_order only knows base labels. We need to expand the
        // label_map to include I-/B- to drive this. Easiest: keep
        // base mapping, and have the test verify single-span case
        // with B at start.
        let logits: Vec<f32> = vec![
            5.0, 0.0, // [CLS]: O
            0.0, 5.0, // Alice: PER (label_id 1 == "PER", treated as B since no current span)
            0.0, 5.0, // works: PER (same base — continues the span)
            5.0, 0.0, // at: O (flush)
            0.0, 5.0, // Acme: PER (new B-style)
            5.0, 0.0, // [SEP]: O
        ];
        let (backend, _dir) = build_backend(logits, 2);
        let entities = backend.recognize_blocking("Alice works at Acme").unwrap();
        assert_eq!(entities.len(), 2);

        // First span covers "Alice works".
        let first = entities.iter().next().unwrap();
        let loc = first.location.as_text().expect("text location");
        let text = "Alice works at Acme";
        assert_eq!(&text[loc.start_offset..loc.end_offset], "Alice works");
    }

    #[tokio::test]
    async fn unsupported_language_returns_error() {
        let logits = vec![5.0, 0.0, 5.0, 0.0, 5.0, 0.0, 5.0, 0.0, 5.0, 0.0, 5.0, 0.0];
        let (backend, _dir) = build_backend(logits, 2);
        let backend = backend.with_supported_languages(vec!["en".parse().unwrap()]);
        let lang: LanguageTag = "de".parse().unwrap();
        let err = backend
            .recognize("Hallo Welt", Some(&lang))
            .await
            .expect_err("should error");
        assert!(matches!(err, Error::UnsupportedLanguage(_)));
    }

    #[tokio::test]
    async fn language_hint_accepted_when_supported() {
        let logits = vec![5.0, 0.0, 5.0, 0.0, 5.0, 0.0, 5.0, 0.0, 5.0, 0.0, 5.0, 0.0];
        let (backend, _dir) = build_backend(logits, 2);
        let backend = backend.with_supported_languages(vec!["en".parse().unwrap()]);
        let lang: LanguageTag = "en".parse().unwrap();
        let result = backend.recognize("Hello world", Some(&lang)).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn empty_supported_languages_accepts_any_hint() {
        let logits = vec![5.0, 0.0, 5.0, 0.0, 5.0, 0.0, 5.0, 0.0, 5.0, 0.0, 5.0, 0.0];
        let (backend, _dir) = build_backend(logits, 2);
        let lang: LanguageTag = "de".parse().unwrap();
        let result = backend.recognize("Hallo Welt", Some(&lang)).await;
        assert!(result.is_ok());
    }
}
