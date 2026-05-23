//! [`LlmContext`]: per-call input to [`LlmRecognizer`].
//!
//! [`LlmRecognizer`]: super::LlmRecognizer

use nvisy_codec::handler::TextData;
use nvisy_ontology::entity::EntityKind;
use uuid::Uuid;

use crate::detection::DetectionContext;

/// Per-call input to [`LlmRecognizer`].
///
/// [`LlmRecognizer`]: super::LlmRecognizer
#[derive(Debug, Clone)]
pub struct LlmContext {
    /// The text to analyze.
    pub text: TextData,
    /// Entity-kind allowlist. Empty = all kinds permitted.
    pub entities: Option<Vec<EntityKind>>,
    /// Minimum confidence threshold in `[0.0, 1.0]`.
    pub score_threshold: Option<f64>,
    /// Correlation UUID propagated through the tracing span.
    pub correlation_id: Option<Uuid>,
}

impl From<&DetectionContext> for LlmContext {
    fn from(ctx: &DetectionContext) -> Self {
        Self {
            text: ctx.text.clone(),
            entities: ctx.entities.clone(),
            score_threshold: ctx.score_threshold,
            correlation_id: ctx.correlation_id,
        }
    }
}
