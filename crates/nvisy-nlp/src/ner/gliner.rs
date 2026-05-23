//! [`GlinerBackend`] — zero-shot NER via the [`gline-rs`] crate.
//!
//! Unlike [`OrtBackend`], whose label vector is baked into the
//! exported ONNX file, GLiNER models accept the entity-label list at
//! inference time. This makes it the only backend that can honor a
//! per-tenant or per-request "detect these kinds for this call"
//! request without retraining.
//!
//! The label space is bridged through [`GlinerConfig::label_map`]:
//! operators declare which GLiNER label string (e.g. `"person"`)
//! maps to which `(EntityCategory, EntityKind)` in our ontology. The
//! `requested_kinds` hint passed to [`NerBackend::recognize`] is then
//! reverse-mapped to the subset of GLiNER label strings to actually
//! ask the model about — keeping inference cost proportional to the
//! number of kinds the caller cares about (the uni-encoder
//! concatenates labels into the input).
//!
//! Caveats: accuracy on jurisdiction-specific structured IDs (e.g.
//! IBAN, SSN, passport numbers) is materially softer than the
//! headline GLiNER benchmarks suggest. Keep regex/checksum
//! recognizers in front of this backend for high-stakes structured
//! identifiers.
//!
//! [`gline-rs`]: https://crates.io/crates/gline-rs
//! [`OrtBackend`]: super::OrtBackend
//! [`NerBackend::recognize`]: super::NerBackend::recognize

use std::collections::HashMap;
use std::fmt;
use std::path::PathBuf;
use std::sync::Arc;

use async_trait::async_trait;
use gliner::model::GLiNER;
use gliner::model::input::text::TextInput;
use gliner::model::params::Parameters;
use gliner::model::pipeline::span::SpanMode;
use gliner::model::pipeline::token::TokenMode;
use nvisy_ontology::entity::{
    Entities, Entity, EntityCategory, EntityKind, Location, ModelKind, RecognitionMethod,
    TextLocation,
};
use nvisy_ontology::primitive::{Confidence, LanguageTag};
use orp::params::RuntimeParameters;

use super::NerBackend;
use crate::error::{Error, Result};

/// Which GLiNER decoding pipeline a model uses.
///
/// Picked when the model is exported; mixing modes silently produces
/// garbage. The HF model card for each preset states which one it
/// targets — `gliner_small/medium/large-v2.1` use [`Span`], the
/// multitask family uses [`Token`].
///
/// [`Span`]: Self::Span
/// [`Token`]: Self::Token
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum GlinerMode {
    /// Span-decoding pipeline (e.g. `gliner_small-v2.1`).
    Span,
    /// Token-decoding pipeline (e.g. `gliner-multitask-large-v0.5`).
    Token,
}

/// Configuration for [`GlinerBackend`].
#[derive(Debug, Clone)]
pub struct GlinerConfig {
    /// Path to the `.onnx` model file.
    pub model_path: PathBuf,
    /// Path to the matching `tokenizer.json`.
    pub tokenizer_path: PathBuf,
    /// Decoding pipeline the exported model targets.
    pub mode: GlinerMode,
    /// Map from GLiNER label string (lowercase, e.g. `"person"`) to
    /// the entity it represents. Labels not present here are dropped
    /// at recognition time, even if the model returns spans for them.
    pub label_map: HashMap<String, (EntityCategory, EntityKind)>,
    /// Model identifier surfaced through [`RecognitionMethod::ner`]
    /// on every produced entity. Useful for provenance tracking.
    pub model_name: String,
}

/// A [`NerBackend`] that runs a GLiNER model via `gline-rs`.
///
/// GLiNER is zero-shot: the entity-label list is supplied at
/// inference time, not at training time. When [`recognize`] is given
/// a `requested_kinds` hint, this backend reverse-maps it through
/// [`GlinerConfig::label_map`] and only asks the model about the
/// matching labels — keeping inference cost proportional to caller
/// intent. With no hint, every label in the configured map is asked
/// about.
///
/// [`recognize`]: NerBackend::recognize
pub struct GlinerBackend {
    state: Arc<GlinerState>,
}

struct GlinerState {
    inner: GlinerInner,
    label_map: HashMap<String, (EntityCategory, EntityKind)>,
    /// Reverse index: `EntityKind` → all GLiNER labels that map to
    /// it. Populated once at construction so per-call filtering
    /// doesn't re-scan `label_map`.
    kind_to_labels: HashMap<EntityKind, Vec<String>>,
    model_name: String,
    supported_languages: Vec<LanguageTag>,
}

/// `gline-rs` keeps the two decoding pipelines as separate types
/// (`GLiNER<SpanMode>` vs `GLiNER<TokenMode>`) — pick at construction.
enum GlinerInner {
    Span(GLiNER<SpanMode>),
    Token(GLiNER<TokenMode>),
}

impl GlinerBackend {
    /// Load the model and tokenizer eagerly.
    pub fn new(config: GlinerConfig) -> Result<Self> {
        if config.label_map.is_empty() {
            return Err(Error::Backend(
                "GlinerConfig.label_map must contain at least one entry".to_owned(),
            ));
        }

        let inner = match config.mode {
            GlinerMode::Span => {
                let model = GLiNER::<SpanMode>::new(
                    Parameters::default(),
                    RuntimeParameters::default(),
                    &config.tokenizer_path,
                    &config.model_path,
                )
                .map_err(|e| Error::ModelLoad {
                    path: config.model_path.clone(),
                    cause: e.to_string(),
                })?;
                GlinerInner::Span(model)
            }
            GlinerMode::Token => {
                let model = GLiNER::<TokenMode>::new(
                    Parameters::default(),
                    RuntimeParameters::default(),
                    &config.tokenizer_path,
                    &config.model_path,
                )
                .map_err(|e| Error::ModelLoad {
                    path: config.model_path.clone(),
                    cause: e.to_string(),
                })?;
                GlinerInner::Token(model)
            }
        };

        let kind_to_labels = build_kind_index(&config.label_map);
        Ok(Self {
            state: Arc::new(GlinerState {
                inner,
                label_map: config.label_map,
                kind_to_labels,
                model_name: config.model_name,
                supported_languages: Vec::new(),
            }),
        })
    }

    /// Set the languages this backend was trained on. Defaults to an
    /// empty list — most GLiNER models are multilingual or
    /// English-only and don't enforce a hint, so the default is
    /// "accept any". A non-empty list rejects unmatched hints with
    /// [`Error::UnsupportedLanguage`].
    pub fn with_supported_languages(mut self, languages: Vec<LanguageTag>) -> Self {
        Arc::get_mut(&mut self.state)
            .expect("with_supported_languages must be called before any clone")
            .supported_languages = languages;
        self
    }

    /// Pick the subset of GLiNER label strings to ask about. If
    /// `requested_kinds` is `None` or empty, all configured labels
    /// are asked; otherwise we ask only the labels mapped to one of
    /// those kinds. Returns an empty `Vec` only when the caller asked
    /// for kinds none of which this backend can produce — the
    /// recognizer can short-circuit in that case.
    fn select_labels(&self, requested_kinds: Option<&[EntityKind]>) -> Vec<String> {
        match requested_kinds {
            Some(kinds) if !kinds.is_empty() => kinds
                .iter()
                .filter_map(|k| self.state.kind_to_labels.get(k))
                .flatten()
                .cloned()
                .collect(),
            _ => self.state.label_map.keys().cloned().collect(),
        }
    }

    fn recognize_blocking(&self, text: &str, labels: &[String]) -> Result<Entities> {
        // `gline-rs` works in batched mode, so we hand it a 1-element
        // slice and read sequence 0 back out.
        let texts: [&str; 1] = [text];
        let label_refs: Vec<&str> = labels.iter().map(String::as_str).collect();

        let input = TextInput::from_str(&texts, &label_refs)
            .map_err(|e| Error::Inference(format!("gliner input build: {e}")))?;

        // `gline-rs` parameterises `GLiNER` over a pipeline marker
        // type, so Span and Token produce different concrete types
        // that must be matched separately. Both expose the same
        // `output.spans` shape downstream.
        let output_spans = match &self.state.inner {
            GlinerInner::Span(model) => model.inference(input).map(|o| o.spans),
            GlinerInner::Token(model) => model.inference(input).map(|o| o.spans),
        }
        .map_err(|e| Error::Inference(format!("gliner inference: {e}")))?;

        Ok(output_spans
            .into_iter()
            .flatten()
            .filter_map(|s| self.build_entity(&s))
            .collect())
    }

    fn build_entity(&self, span: &gliner::text::span::Span) -> Option<Entity> {
        let (category, kind) = self.state.label_map.get(span.class()).copied()?;
        let (start, end) = span.offsets();
        let location = TextLocation::builder()
            .with_start_offset(start)
            .with_end_offset(end)
            .build()
            .ok()?;
        let confidence = Confidence::clamped(f64::from(span.probability()));
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

/// Invert the label-map into `EntityKind` → all GLiNER labels that
/// resolve to it. Used to scope per-call inference to the labels
/// matching `Context::entities`.
fn build_kind_index(
    label_map: &HashMap<String, (EntityCategory, EntityKind)>,
) -> HashMap<EntityKind, Vec<String>> {
    let mut index: HashMap<EntityKind, Vec<String>> = HashMap::new();
    for (label, (_, kind)) in label_map {
        index.entry(*kind).or_default().push(label.clone());
    }
    index
}

impl fmt::Debug for GlinerBackend {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("GlinerBackend")
            .field("model", &self.state.model_name)
            .field(
                "mode",
                match &self.state.inner {
                    GlinerInner::Span(_) => &"span",
                    GlinerInner::Token(_) => &"token",
                },
            )
            .field("labels", &self.state.label_map.len())
            .finish_non_exhaustive()
    }
}

#[async_trait]
impl NerBackend for GlinerBackend {
    async fn recognize(
        &self,
        text: &str,
        language: Option<&LanguageTag>,
        requested_kinds: Option<&[EntityKind]>,
    ) -> Result<Entities> {
        if let Some(lang) = language
            && !self.state.supported_languages.is_empty()
            && !self.state.supported_languages.contains(lang)
        {
            return Err(Error::UnsupportedLanguage(lang.clone()));
        }

        let labels = self.select_labels(requested_kinds);
        if labels.is_empty() {
            // Caller asked only for kinds this backend can't produce
            // — skip the model round-trip entirely.
            return Ok(Entities::new());
        }

        let state = Arc::clone(&self.state);
        let text = text.to_owned();
        tokio::task::spawn_blocking(move || {
            let backend = GlinerBackend { state };
            backend.recognize_blocking(&text, &labels)
        })
        .await
        .map_err(|e| Error::Inference(format!("join error: {e}")))?
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `GlinerBackend::new` rejects an empty `label_map` rather than
    /// constructing a backend that would always ask the model about
    /// zero labels (which `gline-rs` itself would fail on, less
    /// helpfully).
    #[test]
    fn empty_label_map_rejected() {
        let cfg = GlinerConfig {
            model_path: PathBuf::from("/unused.onnx"),
            tokenizer_path: PathBuf::from("/unused.json"),
            mode: GlinerMode::Span,
            label_map: HashMap::new(),
            model_name: "test".to_owned(),
        };
        let err = GlinerBackend::new(cfg).expect_err("empty label map should fail");
        assert!(matches!(err, Error::Backend(_)));
    }

    /// Two GLiNER labels can map to the same `EntityKind` (e.g.
    /// `"location"` and `"address"` both → `GeolocationMetadata`).
    /// The reverse index must collect both under one key so that
    /// asking for that kind activates every matching label.
    #[test]
    fn build_kind_index_groups_aliased_labels() {
        let mut label_map = HashMap::new();
        label_map.insert(
            "location".to_owned(),
            (EntityCategory::Location, EntityKind::GeolocationMetadata),
        );
        label_map.insert(
            "address".to_owned(),
            (EntityCategory::Location, EntityKind::GeolocationMetadata),
        );
        label_map.insert(
            "person".to_owned(),
            (EntityCategory::PersonalIdentity, EntityKind::PersonName),
        );
        let index = build_kind_index(&label_map);

        let geo = index.get(&EntityKind::GeolocationMetadata).unwrap();
        assert_eq!(geo.len(), 2);
        assert!(geo.contains(&"location".to_owned()));
        assert!(geo.contains(&"address".to_owned()));
        assert_eq!(index.get(&EntityKind::PersonName).unwrap().len(), 1);
    }
}
