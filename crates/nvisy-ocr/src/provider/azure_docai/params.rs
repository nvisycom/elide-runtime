use std::fmt;

use serde::{Deserialize, Serialize};

/// Constructor parameters for [`AzureDocaiBackend`].
///
/// [`AzureDocaiBackend`]: super::AzureDocaiBackend
#[derive(Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "config", derive(clap::Args))]
pub struct AzureDocaiParams {
    /// Azure resource endpoint URL.
    #[cfg_attr(feature = "config", arg(long, env = "AZURE_DOCAI_ENDPOINT"))]
    pub endpoint: String,
    /// Azure subscription API key.
    #[cfg_attr(
        feature = "config",
        arg(long, env = "AZURE_DOCAI_API_KEY", hide_env_values = true)
    )]
    pub api_key: String,
    /// API version to use. Defaults to `"2024-11-30"`.
    #[cfg_attr(feature = "config", arg(long, env = "AZURE_DOCAI_API_VERSION"))]
    pub api_version: Option<String>,
    /// Poll interval in milliseconds when waiting for results. Defaults to 500.
    #[cfg_attr(feature = "config", arg(long, env = "AZURE_DOCAI_POLL_INTERVAL_MS"))]
    pub poll_interval_ms: Option<u64>,
    /// Maximum number of poll attempts before timing out. Defaults to 120.
    #[cfg_attr(feature = "config", arg(long, env = "AZURE_DOCAI_MAX_POLL_ATTEMPTS"))]
    pub max_poll_attempts: Option<u32>,
}

impl fmt::Debug for AzureDocaiParams {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("AzureDocaiParams")
            .field("endpoint", &self.endpoint)
            .field("api_key", &"***")
            .field("api_version", &self.api_version)
            .finish()
    }
}
