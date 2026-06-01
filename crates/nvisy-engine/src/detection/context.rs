//! Per-call detection contexts.
//!
//! The base [`DetectionContext`] carries the fields every recognizer
//! reads regardless of modality (entity-kind allowlist, document
//! labels, correlation id). Per-modality contexts compose this base
//! plus their modality-specific payload:
//!
//! - [`TextDetectionContext`]: base + text payload, language hints,
//!   pattern-scan filter, and LLM hints.
//! - [`ImageDetectionContext`]: base + encoded image bytes + pixel
//!   dimensions.
//!
//! Per-modality contexts implement [`Deref`]/[`DerefMut`] back to the
//! base so accessors like `ctx.entities` and `ctx.labels` work
//! transparently on every recognizer's input.
//!
//! `correlation_id` flows through the tracing span and isn't read by
//! recognizer bodies directly.

use bytes::Bytes;
use derive_more::{Deref, DerefMut};
use nvisy_agent::agent::NerHint;
use nvisy_codec::handler::TextData;
use nvisy_ontology::entity::EntityKind;
use nvisy_ontology::primitive::{Dimensions, LanguageTag};
use nvisy_pattern::filter::PatternContext;
use uuid::Uuid;

/// Shared per-call detection context.
///
/// Holds the fields every recognizer reads regardless of modality:
/// the entity-kind allowlist, document-level labels, and the
/// tracing-side correlation id. Per-modality contexts embed this
/// struct and deref to it via [`Deref`].
#[derive(Debug, Default, Clone)]
pub struct DetectionContext {
    /// Entity-kind allowlist. Recognizers that support post-filter
    /// drop entities of any kind outside this set.
    pub entities: Option<Vec<EntityKind>>,

    /// Document-level classification labels forwarded from
    /// [`Document::labels`]. LLM/VLM recognizers render them into the
    /// prompt as context; non-LLM recognizers ignore this field.
    ///
    /// [`Document::labels`]: nvisy_ontology::document::Document::labels
    pub labels: Vec<String>,

    /// Correlation UUID propagated through the tracing span for this
    /// detection call.
    pub correlation_id: Option<Uuid>,
}

/// Per-call input to every text-modality recognizer.
///
/// Composes a shared [`DetectionContext`] base plus the text payload
/// and the text-only filters. Fully owned (no lifetime parameter) so
/// the engine can share it across recognizer tasks via [`Arc`] for
/// parallel dispatch. `text` is a [`TextData`] — internally a
/// `HipStr` — so the shared clone is an atomic increment, not a copy
/// of the source bytes.
///
/// [`Arc`]: std::sync::Arc
#[derive(Debug, Clone, Deref, DerefMut)]
pub struct TextDetectionContext {
    /// Shared base — accessed transparently via [`Deref`].
    #[deref]
    #[deref_mut]
    pub base: DetectionContext,

    /// The text to analyze. Cheap to clone (atomic incr on the inner
    /// `HipStr`, inline for short text).
    pub text: TextData,

    /// Caller-asserted language. When `Some`, NER recognizers skip
    /// per-call language detection.
    pub language: Option<LanguageTag>,

    /// Restrict language detection to this subset. Ignored when
    /// `language` is `Some`.
    pub candidate_languages: Option<Vec<LanguageTag>>,

    /// Allow/deny/hints for pattern-backed recognizers. Non-pattern
    /// recognizers ignore this field.
    pub scan_context: PatternContext,

    /// User-supplied hint regions to fold into the LLM/VLM detector's
    /// prompt for per-hint adjudication alongside open-ended
    /// discovery. Forwarded from [`Document::annotations`]
    /// (`Hint`-strength `Inclusion`). Non-LLM recognizers ignore this
    /// field.
    ///
    /// Exclusion annotations don't flow through this path — they're
    /// always assertions and enforced by a post-detection filter
    /// regardless of recognizer.
    ///
    /// [`Document::annotations`]: nvisy_ontology::document::Document::annotations
    pub hints: Vec<NerHint>,
}

impl TextDetectionContext {
    /// Construct a context with only `text` set; every other field
    /// gets its [`Default`] value.
    pub fn new(text: impl Into<TextData>) -> Self {
        Self {
            base: DetectionContext::default(),
            text: text.into(),
            language: None,
            candidate_languages: None,
            scan_context: PatternContext::default(),
            hints: Vec::new(),
        }
    }
}

/// Per-call input to every image-modality recognizer.
///
/// Composes a shared [`DetectionContext`] base plus the encoded image
/// bytes and their pixel [`Dimensions`] (needed by recognizers that
/// emit normalised bounding boxes — they scale to pixel space using
/// `dims`).
#[derive(Debug, Clone, Deref, DerefMut)]
pub struct ImageDetectionContext {
    /// Shared base — accessed transparently via [`Deref`].
    #[deref]
    #[deref_mut]
    pub base: DetectionContext,

    /// Encoded image bytes (typically PNG).
    pub image: Bytes,

    /// Pixel dimensions of the encoded image.
    pub dims: Dimensions,
}

impl ImageDetectionContext {
    /// Construct a context with the encoded image bytes + their pixel
    /// dimensions. All shared filter fields default to empty.
    pub fn new(image: Bytes, dims: Dimensions) -> Self {
        Self {
            base: DetectionContext::default(),
            image,
            dims,
        }
    }
}
