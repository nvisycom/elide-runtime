//! Synthetic data generation action — fills in realistic replacement values
//! for redactions marked with `Synthesize`.

use serde::Deserialize;

use crate::ontology::Entity;
use crate::redaction::Redaction;
use nvisy_core::Error;

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

impl GenerateSyntheticAction {
    pub async fn connect(_params: GenerateSyntheticParams) -> Result<Self, Error> {
        Ok(Self)
    }

    pub async fn execute(
        &self,
        input: GenerateSyntheticInput,
    ) -> Result<Vec<Redaction>, Error> {
        // Stub: returns redactions unchanged. Real implementation will fill
        // Synthesize variants with generated replacement values.
        Ok(input.redactions)
    }
}
