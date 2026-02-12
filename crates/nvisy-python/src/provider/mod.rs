//! AI provider factory for the Python NER bridge.
//!
//! Registers itself as the `"ai"` provider and yields a [`PythonBridge`]
//! instance upon connection.

use serde::Deserialize;

use nvisy_core::error::Error;
use nvisy_pipeline::provider::{ConnectedInstance, Provider};
use crate::bridge::PythonBridge;

/// Typed credentials for the AI provider.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AiCredentials {
    /// API key forwarded to the AI model provider.
    pub api_key: String,
}

/// Factory that creates [`PythonBridge`] instances from typed credentials.
///
/// The Python interpreter is **not** initialized at connection time; it is
/// lazily loaded on the first NER call.
pub struct AiProvider;

#[async_trait::async_trait]
impl Provider for AiProvider {
    type Credentials = AiCredentials;
    type Client = PythonBridge;

    fn id(&self) -> &str { "ai" }

    fn validate_credentials(&self, _creds: &Self::Credentials) -> Result<(), Error> {
        // api_key is required by the struct, so if we got here it's present.
        Ok(())
    }

    async fn verify(&self, creds: &Self::Credentials) -> Result<(), Error> {
        self.validate_credentials(creds)
    }

    async fn connect(&self, _creds: &Self::Credentials) -> Result<ConnectedInstance<Self::Client>, Error> {
        let bridge = PythonBridge::default();
        // Don't init here — Python might not be available at connect time
        // Init happens lazily when detect_ner is called

        Ok(ConnectedInstance {
            client: bridge,
            disconnect: None,
        })
    }
}
