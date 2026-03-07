use std::fmt;

use serde::{Deserialize, Serialize};

/// Constructor parameters for [`GoogleVisionBackend`].
///
/// [`GoogleVisionBackend`]: super::GoogleVisionBackend
#[derive(Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "config", derive(clap::Args))]
pub struct GoogleVisionParams {
    /// Google Cloud API key.
    #[cfg_attr(
        feature = "config",
        arg(long, env = "GOOGLE_VISION_API_KEY", hide_env_values = true)
    )]
    pub api_key: String,
}

impl fmt::Debug for GoogleVisionParams {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("GoogleVisionParams")
            .field("api_key", &"***")
            .finish()
    }
}
