//! [`NerContext`]: per-call input to [`super::NerRecognizer`].

use nvisy_codec::handler::TextData;
use nvisy_ontology::entity::EntityKind;
use nvisy_ontology::primitive::LanguageTag;
use uuid::Uuid;

use crate::detection::DetectionContext;

/// Per-call input to [`super::NerRecognizer`].
#[derive(Debug, Clone)]
pub struct NerContext {
    /// The text to analyze.
    pub text: TextData,
    /// Caller-asserted language. When `Some`, NER skips per-call
    /// language detection.
    pub language: Option<LanguageTag>,
    /// Restrict language detection to this subset. Ignored when
    /// `language` is `Some`.
    pub candidate_languages: Option<Vec<LanguageTag>>,
    /// Entity-kind allowlist. Empty = all kinds permitted.
    pub entities: Option<Vec<EntityKind>>,
    /// Minimum confidence threshold in `[0.0, 1.0]`.
    pub score_threshold: Option<f64>,
    /// Correlation UUID propagated through the tracing span.
    pub correlation_id: Option<Uuid>,
}

impl From<&DetectionContext> for NerContext {
    fn from(ctx: &DetectionContext) -> Self {
        Self {
            text: ctx.text.clone(),
            language: ctx.language.clone(),
            candidate_languages: ctx.candidate_languages.clone(),
            entities: ctx.entities.clone(),
            score_threshold: ctx.score_threshold,
            correlation_id: ctx.correlation_id,
        }
    }
}
