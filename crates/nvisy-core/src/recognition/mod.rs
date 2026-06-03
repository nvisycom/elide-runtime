//! [`EntityRecognizer<M>`]: the Presidio-style entity-detection trait, and the
//! per-modality [`RecognizerInput`] / [`ModalityData`] shapes recognizers
//! read from.
//!
//! Every detector that emits [`Entity<M>`] for some modality `M`
//! implements this trait — pattern recognizers, NER bento clients,
//! LLM agents, OCR pipelines, plus any third-party recognizer
//! a consumer wires into their pipeline. Object-safe so heterogeneous
//! recognizers live behind `Arc<dyn EntityRecognizer<M>>` in consumer-side
//! registries.
//!
//! # Layering
//!
//! - [`ModalityData`] extends [`crate::modality::Modality`]
//!   with an associated [`Data`] type — the
//!   modality-specific payload (text bytes, image bytes + dims, …)
//!   recognizers actually scan.
//! - [`RecognizerInput<M>`] wraps the payload (`M::Data`) plus the
//!   *shared* per-call concerns every recognizer can read: language
//!   hints, uploader-supplied [`Hint<M>`]s, document-level labels,
//!   correlation id.
//! - [`EntityRecognizer<M>`] takes `&RecognizerInput<M>` and emits a
//!   [`RecognizerOutput<M>`].
//!
//! [`Entity<M>`]: crate::entity::Entity
//! [`Data`]: ModalityData::Data

mod hint;
mod label_map;

use bytes::Bytes;
use hipstr::HipStr;
use type_map::concurrent::TypeMap;
use uuid::Uuid;

pub use self::hint::Hint;
pub use self::label_map::LabelMap;
use crate::Result;
use crate::entity::Entity;
use crate::modality::{Audio, Image, Modality, Text};
use crate::primitive::{Dimensions, LanguageTag};

/// Extension of [`Modality`] that adds the per-call payload type
/// recognizers consume. Modalities that don't (yet) have recognizers
/// — currently `Audio` and `Tabular` — simply don't implement this.
pub trait ModalityData: Modality {
    /// Per-call modality-specific payload: the bytes/text/dimensions
    /// the recognizer actually scans.
    type Data: Send + Sync;
}

/// Per-call input for an [`EntityRecognizer<M>`].
///
/// Bundles the modality-specific [`data`] (e.g. text
/// bytes for [`Text`], image bytes + pixel dims for [`Image`]) with
/// the *shared* per-document concerns every recognizer regardless of
/// modality can read: a language hint, uploader-supplied [`Hint<M>`]
/// regions in modality-native coordinates, document-level labels, and
/// a correlation id for tracing spans.
///
/// Recognizers are free to ignore the shared fields.
///
/// [`data`]: Self::data
#[derive(Debug)]
pub struct RecognizerInput<M: ModalityData> {
    /// Modality-specific payload (text bytes, image bytes + dims, …).
    pub data: M::Data,
    /// Caller-asserted language. When `Some`, recognizers that
    /// support per-call language hinting (typically NER / LLM
    /// backends) skip their own detection.
    pub language: Option<LanguageTag>,
    /// Restrict language auto-detection to this subset when
    /// [`language`] is `None`. Empty means "any".
    ///
    /// [`language`]: Self::language
    pub candidate_languages: Vec<LanguageTag>,
    /// Uploader-supplied hint regions in modality-native coordinates.
    /// Recognizers that support hint adjudication (LLM-based NER, VLM)
    /// read this; recognizers that don't (pattern, dictionary) ignore
    /// it.
    pub hints: Vec<Hint<M>>,
    /// Document-level classification labels (e.g. `"medical"`,
    /// `"gdpr-request"`). Recognizers may use these to bias their
    /// behavior for domain-specific terms; those that don't ignore the
    /// field.
    pub labels: Vec<String>,
    /// Correlation UUID propagated through the tracing span for this
    /// call. Recognizer bodies do not read this directly; it's set
    /// on the span by the caller.
    pub correlation_id: Option<Uuid>,
}

impl<M: ModalityData> RecognizerInput<M> {
    /// Construct an input with only the modality payload set;
    /// language hints, labels, hints, and correlation id default to
    /// empty.
    pub fn new(data: M::Data) -> Self {
        Self {
            data,
            language: None,
            candidate_languages: Vec::new(),
            hints: Vec::new(),
            labels: Vec::new(),
            correlation_id: None,
        }
    }

    /// Set the asserted language.
    #[must_use]
    pub fn with_language(mut self, language: LanguageTag) -> Self {
        self.language = Some(language);
        self
    }

    /// Set the candidate languages for auto-detection.
    #[must_use]
    pub fn with_candidate_languages(mut self, languages: Vec<LanguageTag>) -> Self {
        self.candidate_languages = languages;
        self
    }

    /// Attach uploader-supplied hint regions.
    #[must_use]
    pub fn with_hints(mut self, hints: Vec<Hint<M>>) -> Self {
        self.hints = hints;
        self
    }

    /// Attach document-level classification labels.
    #[must_use]
    pub fn with_labels(mut self, labels: Vec<String>) -> Self {
        self.labels = labels;
        self
    }

    /// Set the correlation id propagated through the tracing span.
    #[must_use]
    pub fn with_correlation_id(mut self, id: Uuid) -> Self {
        self.correlation_id = Some(id);
        self
    }

    /// Whether a recognizer rule scoped to `allowed` languages should
    /// run for this call.
    ///
    /// - An empty `allowed` list means the rule is language-agnostic
    ///   and always runs.
    /// - When `allowed` is non-empty and [`language`] is `Some(_)`,
    ///   the rule runs only if the hint is in the list.
    /// - When [`language`] is `None`, the rule still runs — we can't
    ///   disprove applicability without a hint.
    ///
    /// [`language`]: Self::language
    #[must_use]
    pub fn applies_to_language(&self, allowed: &[LanguageTag]) -> bool {
        if allowed.is_empty() {
            return true;
        }
        match self.language.as_ref() {
            Some(l) => allowed.iter().any(|a| a == l),
            None => true,
        }
    }
}

/// Per-call output of an [`EntityRecognizer`].
///
/// Wraps the emitted entities in a named struct (rather than a bare
/// `Vec<Entity<M>>`) so future per-call metadata — drop counters,
/// telemetry, partial-failure flags — can land alongside without
/// churning every recognizer signature.
#[derive(Debug, Clone)]
pub struct RecognizerOutput<M: Modality> {
    /// Entities the recognizer emitted in modality-local coordinates.
    pub entities: Vec<Entity<M>>,
}

impl<M: Modality> RecognizerOutput<M> {
    /// Construct from the underlying entity list.
    #[must_use]
    pub fn new(entities: Vec<Entity<M>>) -> Self {
        Self { entities }
    }

    /// Empty output — no entities emitted.
    #[must_use]
    pub fn empty() -> Self {
        Self::new(Vec::new())
    }
}

impl<M: Modality> Default for RecognizerOutput<M> {
    fn default() -> Self {
        Self::empty()
    }
}

impl<M: Modality> From<Vec<Entity<M>>> for RecognizerOutput<M> {
    fn from(entities: Vec<Entity<M>>) -> Self {
        Self::new(entities)
    }
}

impl<M: Modality> From<RecognizerOutput<M>> for Vec<Entity<M>> {
    fn from(output: RecognizerOutput<M>) -> Self {
        output.entities
    }
}

/// Recognizer for a single [`Modality`] `M`.
///
/// Implementors emit a [`RecognizerOutput<M>`] for one document or
/// one scan unit, reading whatever per-call configuration they need
/// from [`RecognizerInput<M>`]. Each consumer composes their own list
/// of recognizers; the trait does not assume a central registry.
///
/// Recognizers are stateless from the caller's perspective — the
/// default [`reset`] is a no-op. Long-lived
/// implementations (LLM agents with cumulative usage trackers, OCR
/// backends with batch caches) override `reset` to drop
/// per-document state between runs.
///
/// [`reset`]: Self::reset
#[async_trait::async_trait]
pub trait EntityRecognizer<M: ModalityData>: Send + Sync {
    /// Detect entities in `input` and return them in modality-local
    /// coordinates. Downstream callers rebase text offsets into
    /// document coordinates when stitching results back into a
    /// multi-block document; image entities pass through unchanged.
    async fn recognize(&self, input: &RecognizerInput<M>) -> Result<RecognizerOutput<M>>;

    /// Drop per-document state. Default no-op for stateless
    /// recognizers; long-lived ones (usage trackers, batch caches)
    /// override.
    async fn reset(&self) {}
}

/// Per-call payload for [`Text`] recognizers.
///
/// Held as a [`HipStr<'static>`] so cheap clones (atomic refcount
/// for non-inline text, inline copy for short strings) let the
/// caller share one payload across multiple recognizers without
/// duplicating the source bytes.
///
/// [`artifacts`] is a heterogeneous typed-map (`type_map::TypeMap`)
/// holding shared NLP enrichment produced by an upstream
/// `NlpEngine`. Each enrichment is a distinct typed entry — e.g.
/// [`Tokens`] (lemmatized tokens for the
/// [`ContextEnhancer`]'s lemma matcher),
/// [`LanguageDetections`] (resolved
/// languages), [`StopwordSet`] (per-language stopword list).
/// Consumers do `artifacts.get::<Tokens>()`; when the entry is
/// absent (no engine ran, or engine doesn't produce that
/// enrichment) the consumer silently degrades.
///
/// [`artifacts`]: Self::artifacts
/// [`Tokens`]: crate::nlp::Tokens
/// [`LanguageDetections`]: crate::nlp::LanguageDetections
/// [`StopwordSet`]: crate::nlp::StopwordSet
/// [`ContextEnhancer`]: crate::context::ContextEnhancer
#[derive(Debug, Default)]
pub struct TextData {
    /// The text the recognizer should scan. Byte offsets in emitted
    /// entities refer back into this string.
    pub text: HipStr<'static>,
    /// Shared NLP enrichment produced by the orchestrator's
    /// `NlpEngine` pass, keyed by Rust type. Empty when no shared
    /// pass was run; recognizers that don't care ignore the field.
    pub artifacts: TypeMap,
}

impl TextData {
    /// Construct from anything convertible to [`HipStr<'static>`] —
    /// owned `String`, borrowed `&'static str`, an existing
    /// `HipStr`, …. No artifacts attached; use
    /// [`with_artifacts`] to attach a populated type-map or
    /// [`insert_artifact`] to add one typed entry at a time.
    ///
    /// [`with_artifacts`]: Self::with_artifacts
    /// [`insert_artifact`]: Self::insert_artifact
    pub fn new(text: impl Into<HipStr<'static>>) -> Self {
        Self {
            text: text.into(),
            artifacts: TypeMap::new(),
        }
    }

    /// Replace the artifacts type-map. Use when the orchestrator
    /// produced a fully-populated bundle elsewhere; for incremental
    /// stamping prefer [`insert_artifact`].
    ///
    /// [`insert_artifact`]: Self::insert_artifact
    #[must_use]
    pub fn with_artifacts(mut self, artifacts: TypeMap) -> Self {
        self.artifacts = artifacts;
        self
    }

    /// Insert one typed enrichment entry. Returns the previous
    /// value for that type, if any.
    pub fn insert_artifact<T: Send + Sync + 'static>(&mut self, value: T) -> Option<T> {
        self.artifacts.insert(value)
    }
}

impl ModalityData for Text {
    type Data = TextData;
}

/// Per-call payload for [`Image`] recognizers.
///
/// The pixel dimensions are needed alongside the encoded bytes
/// because recognizers that emit normalised bounding boxes scale
/// them to pixel coordinates using `dims`.
#[derive(Debug, Clone)]
pub struct ImageData {
    /// Encoded image bytes (typically PNG/JPEG).
    pub bytes: Bytes,
    /// Pixel dimensions of the encoded image.
    pub dims: Dimensions,
}

impl ImageData {
    /// Construct with both the encoded bytes and their pixel
    /// dimensions.
    pub fn new(bytes: impl Into<Bytes>, dims: Dimensions) -> Self {
        Self {
            bytes: bytes.into(),
            dims,
        }
    }
}

impl ModalityData for Image {
    type Data = ImageData;
}

/// Per-call payload for [`Audio`] extractors.
///
/// Audio backends (STT, diarization) take encoded bytes plus a
/// filename hint that some providers use to detect the wire format.
/// No dimensions or sample-rate metadata is carried at this layer —
/// providers parse the container themselves.
#[derive(Debug, Clone)]
pub struct AudioData {
    /// Encoded audio bytes (WAV / MP3 / FLAC / …).
    pub bytes: Bytes,
    /// Filename hint passed to providers that key on the extension
    /// to pick a decoder. Falls back to a generic name when the
    /// caller has none.
    pub filename: HipStr<'static>,
}

impl AudioData {
    /// Construct with the encoded bytes and a filename hint.
    pub fn new(bytes: impl Into<Bytes>, filename: impl Into<HipStr<'static>>) -> Self {
        Self {
            bytes: bytes.into(),
            filename: filename.into(),
        }
    }
}

impl ModalityData for Audio {
    type Data = AudioData;
}
