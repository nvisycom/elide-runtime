//! AI provider factory for the Python NER bridge.
//!
//! Registers itself as the `"ai"` provider and yields a [`PythonBridge`]
//! instance upon connection.

use nvisy_core::error::Error;
use nvisy_core::traits::provider::{ConnectedInstance, ProviderFactory};
use crate::bridge::PythonBridge;

/// Factory that creates [`PythonBridge`] instances from JSON credentials.
///
/// Expected credential keys:
/// - `apiKey` (required) -- the API key forwarded to the AI model provider.
///
/// The Python interpreter is **not** initialized at connection time; it is
/// lazily loaded on the first NER call.
pub struct AiProviderFactory;

#[async_trait::async_trait]
impl ProviderFactory for AiProviderFactory {
    fn id(&self) -> &str { "ai" }

    fn validate_credentials(&self, creds: &serde_json::Value) -> Result<(), Error> {
        if creds.get("apiKey").and_then(|v| v.as_str()).is_none() {
            return Err(Error::validation("Missing 'apiKey' in AI credentials", "ai"));
        }
        Ok(())
    }

    async fn verify(&self, creds: &serde_json::Value) -> Result<(), Error> {
        self.validate_credentials(creds)
    }

    async fn connect(&self, _creds: &serde_json::Value) -> Result<ConnectedInstance, Error> {
        let bridge = PythonBridge::default();
        // Don't init here — Python might not be available at connect time
        // Init happens lazily when detect_ner is called

        Ok(ConnectedInstance {
            client: Box::new(bridge),
            disconnect: None,
        })
    }
}
