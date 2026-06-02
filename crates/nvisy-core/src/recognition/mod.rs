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
//! - [`RecognizerInput<D>`] wraps the payload plus *shared* per-call
//!   concerns every recognizer can read: language hints, the correlation
//!   id used by tracing. Whatever's universal across recognizer types
//!   for one call lives here.
//! - [`EntityRecognizer<M>`] takes `&RecognizerInput<M::Data>` and emits
//!   entities.
//!
//! [`Entity<M>`]: crate::entity::Entity
//! [`Data`]: ModalityData::Data

use std::sync::Arc;

use bytes::Bytes;
use hipstr::HipStr;
use uuid::Uuid;

use crate::Result;
use crate::entity::Entity;
use crate::modality::{Audio, Image, Modality, Text};
use crate::nlp::NlpArtifacts;
use crate::primitive::{Dimensions, LanguageTag};

/// Extension of [`Modality`] that adds the per-call payload type
/// recognizers consume. Modalities that don't (yet) have recognizers
/// — currently `Audio` and `Tabular` — simply don't implement this.
pub trait ModalityData: Modality {
    /// Per-call modality-specific payload: the bytes/text/dimensions
    /// the recognizer actually scans.
    type Data: Send + Sync;
}

/// Per-call input for an [`EntityRecognizer`].
///
/// Bundles the modality-specific [`data`] (e.g. text
/// bytes for [`Text`], image bytes + pixel dims for [`Image`]) with
/// the *shared* concerns every recognizer regardless of modality
/// can read: a language hint, candidate languages, and a correlation
/// id for tracing spans.
///
/// Recognizers are free to ignore the shared fields.
///
/// [`data`]: Self::data
#[derive(Debug, Clone)]
pub struct RecognizerInput<D> {
    /// Modality-specific payload (text bytes, image bytes + dims, …).
    pub data: D,
    /// Caller-asserted language. When `Some`, recognizers that
    /// support per-call language hinting (typically NER / LLM
    /// backends) skip their own detection.
    pub language: Option<LanguageTag>,
    /// Restrict language auto-detection to this subset when
    /// [`language`] is `None`. Empty means "any".
    ///
    /// [`language`]: Self::language
    pub candidate_languages: Vec<LanguageTag>,
    /// Correlation UUID propagated through the tracing span for this
    /// call. Recognizer bodies do not read this directly; it's set
    /// on the span by the caller.
    pub correlation_id: Option<Uuid>,
}

impl<D> RecognizerInput<D> {
    /// Construct a context with only the modality payload set;
    /// language hints and correlation id default to empty.
    pub fn new(data: D) -> Self {
        Self {
            data,
            language: None,
            candidate_languages: Vec::new(),
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

/// Recognizer for a single [`Modality`] `M`.
///
/// Implementors emit [`Entity<M>`] values for one document or one
/// scan unit, reading whatever per-call configuration they need from
/// [`RecognizerInput<M::Data>`]. Each consumer composes their own list
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
    /// Detect entities in `ctx` and return them in modality-local
    /// coordinates. Downstream callers rebase text offsets into
    /// document coordinates when stitching results back into a
    /// multi-block document; image entities pass through unchanged.
    async fn recognize(&self, ctx: &RecognizerInput<M::Data>) -> Result<Vec<Entity<M>>>;

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
/// [`artifacts`] is the shared-NLP-pass opt-in.
/// When the orchestrator pre-ran an `NlpEngine`, it wraps the
/// result in an `Arc` and stamps it here so every text recognizer
/// reads the same tokens, lemmas, language detections, and NER
/// spans from one source of truth. Recognizers that don't need
/// artifacts (most patterns) ignore the field; recognizers that
/// require them (NER adapter) error when it's absent.
///
/// [`artifacts`]: Self::artifacts
#[derive(Debug, Clone)]
pub struct TextData {
    /// The text the recognizer should scan. Byte offsets in emitted
    /// entities refer back into this string.
    pub text: HipStr<'static>,
    /// Shared NLP artifacts produced by the orchestrator's
    /// `NlpEngine` pass. `None` when no shared pass was run; in
    /// that case lemma-dependent code paths fall back to substring
    /// scans against [`text`].
    ///
    /// [`text`]: Self::text
    pub artifacts: Option<Arc<NlpArtifacts>>,
}

impl TextData {
    /// Construct from anything convertible to [`HipStr<'static>`] —
    /// owned `String`, borrowed `&'static str`, an existing
    /// `HipStr`, …. No artifacts attached; use
    /// [`with_artifacts`] to attach them.
    ///
    /// [`with_artifacts`]: Self::with_artifacts
    pub fn new(text: impl Into<HipStr<'static>>) -> Self {
        Self {
            text: text.into(),
            artifacts: None,
        }
    }

    /// Attach shared NLP artifacts. The orchestrator wraps its
    /// `NlpEngine::process` output in an `Arc` and stamps it here
    /// so every recognizer reads the same tokens / lemmas / NER
    /// spans.
    #[must_use]
    pub fn with_artifacts(mut self, artifacts: Arc<NlpArtifacts>) -> Self {
        self.artifacts = Some(artifacts);
        self
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
