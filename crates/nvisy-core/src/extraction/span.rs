//! [`Span<M>`]: per-call input handed to an [`Extractor<M>`].
//!
//! Pairs the modality payload with where in the source it lives
//! ([`M::Location`]) and a typed [`Artifacts`] bundle for
//! out-of-band enrichments. Sibling to [`RecognizerInput<M>`] on the
//! recognizer side — the two stay separate while their concerns
//! diverge: recognizers carry hints/labels/candidate languages,
//! extractors carry a location and a typed artifacts bundle.
//!
//! [`Extractor<M>`]: crate::extraction::Extractor
//! [`M::Location`]: crate::modality::Modality::Location
//! [`RecognizerInput<M>`]: crate::recognition::RecognizerInput

use uuid::Uuid;

use super::Artifacts;
use crate::modality::Modality;
use crate::primitive::LanguageTag;

/// Per-call extraction input: the payload, where it lives in the
/// source, an optional language assertion / correlation id, and a
/// typed bundle of [`Artifacts`].
///
/// Modalities that recognizers care about (today: every recognizer
/// modality) implement [`Modality`]; extractors reuse the same
/// `M::Data` shape rather than defining a parallel payload type.
#[derive(Debug)]
pub struct Span<M: Modality> {
    /// Modality-specific payload the extractor will process.
    pub data: M::Data,
    /// Where this payload lives in the source (whole-image for OCR,
    /// full-stream time span for STT, …). Modality-specific via
    /// [`Modality::Location`].
    pub location: M::Location,
    /// Caller-asserted language. Backends that support per-call
    /// language hinting use this; the rest ignore it.
    pub language: Option<LanguageTag>,
    /// Correlation UUID propagated through the tracing span for this
    /// call.
    pub correlation_id: Option<Uuid>,
    /// Heterogeneous typed bundle of per-span enrichments. Empty by
    /// default.
    pub artifacts: Artifacts,
}

impl<M: Modality> Span<M> {
    /// Construct a span with the payload and location set;
    /// language, correlation id, and artifacts default to empty.
    pub fn new(data: M::Data, location: M::Location) -> Self {
        Self {
            data,
            location,
            language: None,
            correlation_id: None,
            artifacts: Artifacts::new(),
        }
    }

    /// Set the asserted language.
    #[must_use]
    pub fn with_language(mut self, language: LanguageTag) -> Self {
        self.language = Some(language);
        self
    }

    /// Set the correlation id propagated through the tracing span.
    #[must_use]
    pub fn with_correlation_id(mut self, id: Uuid) -> Self {
        self.correlation_id = Some(id);
        self
    }

    /// Replace the artifacts bundle with `artifacts`.
    #[must_use]
    pub fn with_artifacts(mut self, artifacts: Artifacts) -> Self {
        self.artifacts = artifacts;
        self
    }
}
