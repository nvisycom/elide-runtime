//! [`Recognizer<M>`]: the Presidio-style entity-detection trait, and the
//! per-modality [`Context`] / [`ModalityData`] shapes recognizers read from.
//!
//! Every detector that emits [`Entity<M>`] for some modality `M`
//! implements this trait — pattern recognizers, NER bento clients,
//! LLM agents, OCR pipelines, plus any third-party recognizer
//! a consumer wires into their pipeline. Object-safe so heterogeneous
//! recognizers live behind `Arc<dyn Recognizer<M>>` in consumer-side
//! registries.
//!
//! # Layering
//!
//! - [`ModalityData`] extends [`nvisy_ontology::modality::Modality`]
//!   with an associated [`Data`](ModalityData::Data) type — the
//!   modality-specific payload (text bytes, image bytes + dims, …)
//!   recognizers actually scan.
//! - [`Context<D>`] wraps the payload plus *shared* per-call concerns
//!   every recognizer can read: language hints, the correlation id
//!   used by tracing. Whatever's universal across recognizer types
//!   for one call lives here.
//! - [`Recognizer<M>`] takes `&Context<M::Data>` and emits entities.
//!
//! [`Entity<M>`]: nvisy_ontology::entity::Entity

use bytes::Bytes;
use hipstr::HipStr;
use nvisy_ontology::entity::Entity;
use nvisy_ontology::modality::{Image, Modality, Text};
use nvisy_ontology::primitive::{Dimensions, LanguageTag};
use uuid::Uuid;

use crate::Result;

/// Extension of [`Modality`] that adds the per-call payload type
/// recognizers consume. Modalities that don't (yet) have recognizers
/// — currently `Audio` and `Tabular` — simply don't implement this.
pub trait ModalityData: Modality {
    /// Per-call modality-specific payload: the bytes/text/dimensions
    /// the recognizer actually scans.
    type Data: Send + Sync;
}

/// Per-call input for a [`Recognizer`].
///
/// Bundles the modality-specific [`data`](Self::data) (e.g. text
/// bytes for [`Text`], image bytes + pixel dims for [`Image`]) with
/// the *shared* concerns every recognizer regardless of modality
/// can read: a language hint, candidate languages, and a correlation
/// id for tracing spans.
///
/// Recognizers are free to ignore the shared fields; pattern
/// recognizers in particular don't care about language.
#[derive(Debug, Clone)]
pub struct Context<D> {
    /// Modality-specific payload (text bytes, image bytes + dims, …).
    pub data: D,
    /// Caller-asserted language. When `Some`, recognizers that
    /// support per-call language hinting (typically NER / LLM
    /// backends) skip their own detection.
    pub language: Option<LanguageTag>,
    /// Restrict language auto-detection to this subset when
    /// [`language`](Self::language) is `None`. Empty means "any".
    pub candidate_languages: Vec<LanguageTag>,
    /// Correlation UUID propagated through the tracing span for this
    /// call. Recognizer bodies do not read this directly; it's set
    /// on the span by the caller.
    pub correlation_id: Option<Uuid>,
}

impl<D> Context<D> {
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
}

/// Recognizer for a single [`Modality`] `M`.
///
/// Implementors emit [`Entity<M>`] values for one document or one
/// scan unit, reading whatever per-call configuration they need from
/// [`Context<M::Data>`]. Each consumer composes their own list of
/// recognizers; the trait does not assume a central registry.
///
/// Recognizers are stateless from the caller's perspective — the
/// default [`reset`](Self::reset) is a no-op. Long-lived
/// implementations (LLM agents with cumulative usage trackers, OCR
/// backends with batch caches) override `reset` to drop
/// per-document state between runs.
#[async_trait::async_trait]
pub trait Recognizer<M: ModalityData>: Send + Sync {
    /// Detect entities in `ctx` and return them in modality-local
    /// coordinates. Downstream callers rebase text offsets into
    /// document coordinates when stitching results back into a
    /// multi-block document; image entities pass through unchanged.
    async fn recognize(&self, ctx: &Context<M::Data>) -> Result<Vec<Entity<M>>>;

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
#[derive(Debug, Clone)]
pub struct TextData {
    /// The text the recognizer should scan. Byte offsets in emitted
    /// entities refer back into this string.
    pub text: HipStr<'static>,
}

impl TextData {
    /// Construct from anything convertible to [`HipStr<'static>`] —
    /// owned `String`, borrowed `&'static str`, an existing
    /// `HipStr`, …
    pub fn new(text: impl Into<HipStr<'static>>) -> Self {
        Self { text: text.into() }
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
