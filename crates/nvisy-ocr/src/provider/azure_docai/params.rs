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
}

impl fmt::Debug for AzureDocaiParams {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("AzureDocaiParams")
            .field("endpoint", &self.endpoint)
            .field("api_key", &"***")
            .finish()
    }
}
