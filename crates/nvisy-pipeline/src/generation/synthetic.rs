//! Synthetic data generation action — fills in realistic replacement values
//! for redactions marked with `Synthesize`.

use serde::Deserialize;

use crate::ontology::redaction::Redaction;
use crate::ontology::entity::Entity;
use nvisy_core::error::Error;

use crate::action::Action;

fn default_locale() -> String {
    "en-US".into()
}

/// Typed parameters for [`GenerateSyntheticAction`].
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GenerateSyntheticParams {
    /// BCP-47 locale for synthetic value generation.
    #[serde(default = "default_locale")]
    pub locale: String,
}

/// Typed input for [`GenerateSyntheticAction`].
pub struct GenerateSyntheticInput {
    /// The entities whose redactions need synthetic values.
    pub entities: Vec<Entity>,
    /// The redaction instructions (some may have `Synthesize` outputs).
    pub redactions: Vec<Redaction>,
}

/// Synthetic data generation stub — fills `Synthesize` redaction outputs
/// with realistic replacement values at runtime.
pub struct GenerateSyntheticAction;

#[async_trait::async_trait]
impl Action for GenerateSyntheticAction {
    type Params = GenerateSyntheticParams;
    type Input = GenerateSyntheticInput;
    type Output = Vec<Redaction>;

    fn id(&self) -> &str {
        "generate-synthetic"
    }

    async fn connect(_params: Self::Params) -> Result<Self, Error> {
        Ok(Self)
    }

    async fn execute(
        &self,
        input: Self::Input,
    ) -> Result<Vec<Redaction>, Error> {
        // Stub: returns redactions unchanged. Real implementation will fill
        // Synthesize variants with generated replacement values.
        Ok(input.redactions)
    }
}
