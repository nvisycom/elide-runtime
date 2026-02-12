//! Sensitivity classification action.

use nvisy_ontology::ontology::entity::Entity;
use nvisy_core::error::Error;

use crate::action::Action;

/// Result of sensitivity classification.
pub struct ClassificationResult {
    /// The computed sensitivity level (`"none"`, `"low"`, `"medium"`, `"high"`, or `"critical"`).
    pub sensitivity_level: String,
    /// Total number of entities considered.
    pub total_entities: usize,
}

/// Assigns a sensitivity level based on detected entities.
///
/// The action inspects the entities, computes a sensitivity level
/// (`"none"`, `"low"`, `"medium"`, `"high"`, or `"critical"`), and returns
/// a [`ClassificationResult`].
pub struct ClassifyAction;

#[async_trait::async_trait]
impl Action for ClassifyAction {
    type Params = ();
    type Input = Vec<Entity>;
    type Output = ClassificationResult;

    fn id(&self) -> &str {
        "classify"
    }

    async fn connect(_params: Self::Params) -> Result<Self, Error> {
        Ok(Self)
    }

    async fn execute(
        &self,
        entities: Self::Input,
    ) -> Result<ClassificationResult, Error> {
        let total_entities = entities.len();
        let sensitivity_level = compute_sensitivity_level(&entities);

        Ok(ClassificationResult {
            sensitivity_level,
            total_entities,
        })
    }
}

/// Computes a sensitivity level string from a set of detected entities.
///
/// The heuristic is:
/// - `"none"` -- no entities.
/// - `"critical"` -- at least one high-confidence (>= 0.9) credential, SSN, or credit card.
/// - `"high"` -- any critical type present, or more than 10 entities total.
/// - `"medium"` -- more than 3 entities.
/// - `"low"` -- 1-3 non-critical entities.
fn compute_sensitivity_level(entities: &[Entity]) -> String {
    if entities.is_empty() {
        return "none".to_string();
    }

    let has_high_confidence = entities.iter().any(|e| e.confidence >= 0.9);
    let has_critical_types = entities.iter().any(|e| {
        matches!(
            e.category,
            nvisy_ontology::ontology::entity::EntityCategory::Credentials
        ) || e.entity_type == "ssn"
            || e.entity_type == "credit_card"
    });

    if has_critical_types && has_high_confidence {
        return "critical".to_string();
    }
    if has_critical_types || entities.len() > 10 {
        return "high".to_string();
    }
    if entities.len() > 3 {
        return "medium".to_string();
    }
    "low".to_string()
}
