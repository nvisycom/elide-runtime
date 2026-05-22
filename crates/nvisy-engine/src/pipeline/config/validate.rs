//! Post-deserialization validation.

use nvisy_core::Error;
use validator::Validate;

use super::RuntimeConfig;

impl RuntimeConfig {
    /// Validate all configuration sections.
    ///
    /// Checks structural constraints (e.g. retry/timeout ranges)
    /// using the `validator` crate. Should be called once after
    /// deserialization and after any merge.
    ///
    /// # Errors
    ///
    /// Returns a validation error listing all constraint violations.
    pub fn validate(&self) -> Result<(), Error> {
        if let Some(ref engine) = self.engine {
            engine
                .validate()
                .map_err(|e| Error::validation(format!("engine: {e}"), "config"))?;
        }
        Ok(())
    }

    /// Resolve `api_key` fields from environment variables.
    ///
    /// Placeholder: per-extractor/per-recognizer provider configs
    /// will get their own env-var resolution path in a follow-up.
    pub fn resolve_env(&mut self) {}
}
