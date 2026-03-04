use std::fmt;

/// Constructor parameters for [`AzureDocaiBackend`].
///
/// [`AzureDocaiBackend`]: super::AzureDocaiBackend
#[derive(Clone)]
pub struct AzureDocaiParams {
    /// Azure resource endpoint URL.
    pub endpoint: String,
    /// Azure subscription API key.
    pub api_key: String,
    /// API version to use. Defaults to `"2024-11-30"`.
    pub api_version: Option<String>,
    /// Poll interval in milliseconds when waiting for results. Defaults to 500.
    pub poll_interval_ms: Option<u64>,
    /// Maximum number of poll attempts before timing out. Defaults to 120.
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
