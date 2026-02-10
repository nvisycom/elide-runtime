//! Sensitivity classification action.

use tokio::sync::mpsc;

use nvisy_core::datatypes::blob::Blob;
use nvisy_core::ontology::entity::Entity;
use nvisy_core::error::{Error, ErrorKind};
use nvisy_core::registry::action::Action;

/// Assigns a sensitivity level to each blob based on its detected entities.
///
/// The action inspects the `"entities"` artifact, computes a sensitivity level
/// (`"none"`, `"low"`, `"medium"`, `"high"`, or `"critical"`), and writes it
/// into the blob metadata as `"sensitivityLevel"`. It also records the
/// `"totalEntities"` count.
pub struct ClassifyAction;

#[async_trait::async_trait]
impl Action for ClassifyAction {
    type Params = ();

    fn id(&self) -> &str {
        "classify"
    }

    fn validate_params(&self, _params: &Self::Params) -> Result<(), Error> {
        Ok(())
    }

    async fn execute(
        &self,
        mut input: mpsc::Receiver<Blob>,
        output: mpsc::Sender<Blob>,
        _params: Self::Params,
    ) -> Result<u64, Error> {
        let mut count = 0u64;

        while let Some(mut blob) = input.recv().await {
            let entities: Vec<Entity> = blob.get_artifacts("entities").map_err(|e| {
                Error::new(ErrorKind::Runtime, format!("failed to read entities artifact: {e}"))
            })?;

            let sensitivity_level = compute_sensitivity_level(&entities);

            let mut meta = blob.data.metadata.clone().unwrap_or_default();
            meta.insert(
                "sensitivityLevel".to_string(),
                serde_json::Value::String(sensitivity_level),
            );
            meta.insert(
                "totalEntities".to_string(),
                serde_json::Value::Number(entities.len().into()),
            );
            blob.data.metadata = Some(meta);

            count += 1;
            if output.send(blob).await.is_err() {
                return Ok(count);
            }
        }

        Ok(count)
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
        matches!(e.category, nvisy_core::ontology::entity::EntityCategory::Credentials)
            || e.entity_type == "ssn"
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
