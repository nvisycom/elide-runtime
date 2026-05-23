//! [`PatternContext`]: per-call input to [`PatternRecognizer`].
//!
//! [`PatternRecognizer`]: super::PatternRecognizer

use nvisy_codec::handler::TextData;
use nvisy_ontology::entity::EntityKind;
use nvisy_pattern::filter::ScanContext;
use uuid::Uuid;

use crate::detection::DetectionContext;

/// Per-call input to [`PatternRecognizer`].
///
/// Derived from a [`DetectionContext`] via `From<&DetectionContext>`
/// in the bridge layer.
///
/// [`PatternRecognizer`]: super::PatternRecognizer
#[derive(Debug, Clone)]
pub struct PatternContext {
    /// The text to scan.
    pub text: TextData,
    /// Allow / deny / hints for this scan.
    pub scan_context: ScanContext,
    /// Entity-kind allowlist. Empty = all kinds permitted.
    pub entities: Option<Vec<EntityKind>>,
    /// Minimum confidence threshold in `[0.0, 1.0]`.
    pub score_threshold: Option<f64>,
    /// Correlation UUID propagated through the tracing span.
    pub correlation_id: Option<Uuid>,
}

impl From<&DetectionContext> for PatternContext {
    fn from(ctx: &DetectionContext) -> Self {
        Self {
            text: ctx.text.clone(),
            scan_context: ctx.scan_context.clone(),
            entities: ctx.entities.clone(),
            score_threshold: ctx.score_threshold,
            correlation_id: ctx.correlation_id,
        }
    }
}
