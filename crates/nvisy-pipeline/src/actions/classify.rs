//! Sensitivity classification action.

pub use nvisy_ontology::detection::ClassificationResult;
use nvisy_ontology::detection::SensitivityLevel;
use nvisy_ontology::entity::Entity;
use nvisy_core::error::Error;

use crate::action::Action;

/// Assigns a sensitivity level based on detected entities.
///
/// The action inspects the entities, computes a [`SensitivityLevel`], and
/// returns a [`ClassificationResult`].
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
            risk_score: None,
        })
    }
}

/// Computes a sensitivity level from a set of detected entities.
///
/// The heuristic is:
/// - [`Public`](SensitivityLevel::Public) — no entities.
/// - [`Restricted`](SensitivityLevel::Restricted) — at least one high-confidence (>= 0.9) credential, SSN, or credit card.
/// - [`Confidential`](SensitivityLevel::Confidential) — any critical type present, or more than 10 entities total.
/// - [`Internal`](SensitivityLevel::Internal) — 1–10 non-critical entities.
fn compute_sensitivity_level(entities: &[Entity]) -> SensitivityLevel {
    if entities.is_empty() {
        return SensitivityLevel::Public;
    }

    let has_high_confidence = entities.iter().any(|e| e.confidence >= 0.9);
    let has_critical_types = entities.iter().any(|e| {
        matches!(
            e.category,
            nvisy_ontology::entity::EntityCategory::Credentials
        ) || e.entity_type == "ssn"
            || e.entity_type == "credit_card"
    });

    if has_critical_types && has_high_confidence {
        return SensitivityLevel::Restricted;
    }
    if has_critical_types || entities.len() > 10 {
        return SensitivityLevel::Confidential;
    }
    SensitivityLevel::Internal
}
