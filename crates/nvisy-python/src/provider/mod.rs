use async_trait::async_trait;
use nvisy_core::errors::NvisyError;
use nvisy_core::traits::provider::{ConnectedInstance, ProviderFactory};
use crate::bridge::PythonBridge;

/// AI provider factory that creates PythonBridge instances.
pub struct AiProviderFactory;

#[async_trait]
impl ProviderFactory for AiProviderFactory {
    fn id(&self) -> &str { "ai" }

    fn validate_credentials(&self, creds: &serde_json::Value) -> Result<(), NvisyError> {
        if creds.get("apiKey").and_then(|v| v.as_str()).is_none() {
            return Err(NvisyError::validation("Missing 'apiKey' in AI credentials", "ai"));
        }
        Ok(())
    }

    async fn verify(&self, creds: &serde_json::Value) -> Result<(), NvisyError> {
        self.validate_credentials(creds)
    }

    async fn connect(&self, _creds: &serde_json::Value) -> Result<ConnectedInstance, NvisyError> {
        let bridge = PythonBridge::default();
        // Don't init here — Python might not be available at connect time
        // Init happens lazily when detect_ner is called

        Ok(ConnectedInstance {
            client: Box::new(bridge),
            disconnect: None,
        })
    }
}
