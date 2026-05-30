//! [`DetectionContext`] — per-call input to every text-modality
//! [`Recognizer`].
//!
//! Bundles the same shape `nvisy_ner::Context` carries, plus the
//! [`PatternContext`] needed by pattern-backed recognizers. Each
//! recognizer reads the subset it cares about:
//!
//! - [`NerRecognizer`] honors `text`, `language`,
//!   `candidate_languages`, `entities`.
//! - [`PatternRecognizer`] reads `text` and `scan_context`
//!   (allow/deny/hints).
//! - The LLM recognizer (an [`LlmNerPipeline`]) reads `text`, `hints`,
//!   `labels`, `entities`, `score_threshold`, and `correlation_id`
//!   via the `From<&DetectionContext>` impl on [`LlmNerScanInput`].
//!
//! `correlation_id` flows through the tracing span and isn't read
//! by recognizers themselves.
//!
//! Image-modality recognizers consume the sibling
//! [`VlmDetectionContext`] instead.
//!
//! [`Recognizer`]: super::Recognizer
//! [`NerRecognizer`]: super::NerRecognizer
//! [`PatternRecognizer`]: super::PatternRecognizer
//! [`LlmNerPipeline`]: nvisy_agent::pipeline::LlmNerPipeline
//! [`LlmNerScanInput`]: super::llm::LlmNerScanInput

use bytes::Bytes;
use derive_builder::Builder;
use nvisy_agent::agent::NerHint;
use nvisy_codec::handler::TextData;
use nvisy_ontology::entity::EntityKind;
use nvisy_ontology::primitive::{ConfidenceThreshold, Dimensions, LanguageTag};
use nvisy_pattern::filter::PatternContext;
use uuid::Uuid;

/// Per-call input to every text-modality recognizer.
///
/// Fully owned (no lifetime parameter) so the engine can share it
/// across recognizer tasks via [`Arc`] for parallel dispatch.
/// `text` is a [`TextData`] — internally a `HipStr` — so the
/// shared clone is an atomic increment, not a copy of the source
/// bytes.
///
/// [`Arc`]: std::sync::Arc
#[derive(Debug, Clone, Builder)]
#[builder(
    name = "DetectionContextBuilder",
    pattern = "owned",
    setter(into, strip_option, prefix = "with"),
    build_fn(error = "DetectionContextBuilderError")
)]
pub struct DetectionContext {
    /// The text to analyze. Cheap to clone (atomic incr on the
    /// inner `HipStr`, inline for short text).
    #[builder(setter(into))]
    pub text: TextData,

    /// Caller-asserted language. When `Some`, NER recognizers skip
    /// per-call language detection.
    #[builder(default)]
    pub language: Option<LanguageTag>,

    /// Restrict language detection to this subset. Ignored when
    /// `language` is `Some`.
    #[builder(default)]
    pub candidate_languages: Option<Vec<LanguageTag>>,

    /// Entity-kind allowlist. Recognizers that support post-filter
    /// drop entities of any kind outside this set.
    #[builder(default)]
    pub entities: Option<Vec<EntityKind>>,

    /// Minimum confidence threshold. Recognizers that support
    /// post-filter drop entities below this score.
    #[builder(default)]
    pub score_threshold: Option<ConfidenceThreshold>,

    /// Allow/deny/hints for pattern-backed recognizers.
    /// Non-pattern recognizers ignore this field.
    #[builder(default)]
    pub scan_context: PatternContext,

    /// User-supplied hint regions to fold into the LLM/VLM
    /// detector's prompt for per-hint adjudication alongside
    /// open-ended discovery. Forwarded from
    /// [`Document::annotations`] (`Hint`-strength `Inclusion`).
    /// Non-LLM recognizers ignore this field.
    ///
    /// Exclusion annotations don't flow through this path — they're
    /// always assertions and enforced by a post-detection filter
    /// regardless of recognizer.
    ///
    /// [`Document::annotations`]: nvisy_ontology::document::Document::annotations
    #[builder(default)]
    pub hints: Vec<NerHint>,

    /// Document-level classification labels forwarded from
    /// [`Document::labels`]. LLM/VLM recognizers render them into
    /// the prompt as context; non-LLM recognizers ignore this
    /// field.
    ///
    /// [`Document::labels`]: nvisy_ontology::document::Document::labels
    #[builder(default)]
    pub labels: Vec<String>,

    /// Correlation UUID propagated through the tracing span for
    /// this detection call.
    #[builder(default)]
    pub correlation_id: Option<Uuid>,
}

impl DetectionContext {
    /// Construct a context with only `text` set.
    pub fn new(text: impl Into<TextData>) -> Self {
        Self {
            text: text.into(),
            language: None,
            candidate_languages: None,
            entities: None,
            score_threshold: None,
            scan_context: PatternContext::default(),
            hints: Vec::new(),
            labels: Vec::new(),
            correlation_id: None,
        }
    }

    /// Start a typed builder. Equivalent to
    /// `DetectionContextBuilder::default()` but more discoverable
    /// from the context type.
    pub fn builder() -> DetectionContextBuilder {
        DetectionContextBuilder::default()
    }
}

impl From<TextData> for DetectionContext {
    fn from(text: TextData) -> Self {
        Self::new(text)
    }
}

impl From<&str> for DetectionContext {
    fn from(text: &str) -> Self {
        Self::new(TextData::from(text))
    }
}

impl From<String> for DetectionContext {
    fn from(text: String) -> Self {
        Self::new(TextData::from(text))
    }
}

/// Error returned by [`DetectionContextBuilder::build`] when a
/// required field is missing.
#[derive(Debug, thiserror::Error)]
#[error("DetectionContext build failed: {0}")]
pub struct DetectionContextBuilderError(String);

impl From<derive_builder::UninitializedFieldError> for DetectionContextBuilderError {
    fn from(err: derive_builder::UninitializedFieldError) -> Self {
        Self(format!("missing required field `{}`", err.field_name()))
    }
}

/// Per-call input for image-modality recognizers.
///
/// Image counterpart to [`DetectionContext`]: carries the encoded
/// image bytes plus their pixel [`Dimensions`] (needed by
/// recognizers that emit normalised bounding boxes — they scale to
/// pixel space using `dims`) plus the same filter knobs the text
/// side carries.
#[derive(Debug, Clone)]
pub struct VlmDetectionContext {
    /// Encoded image bytes (typically PNG).
    pub image: Bytes,
    /// Pixel dimensions of the encoded image.
    pub dims: Dimensions,
    /// Entity-kind allowlist forwarded to image recognizers.
    pub entities: Option<Vec<EntityKind>>,
    /// Minimum confidence threshold forwarded to image recognizers.
    pub score_threshold: Option<ConfidenceThreshold>,
    /// Document-level classification labels.
    pub labels: Vec<String>,
    /// Correlation UUID propagated through the tracing span.
    pub correlation_id: Option<Uuid>,
}

impl VlmDetectionContext {
    /// Construct a context with the encoded image bytes + their
    /// pixel dimensions. All filter fields default to empty.
    pub fn new(image: Bytes, dims: Dimensions) -> Self {
        Self {
            image,
            dims,
            entities: None,
            score_threshold: None,
            labels: Vec::new(),
            correlation_id: None,
        }
    }
}
