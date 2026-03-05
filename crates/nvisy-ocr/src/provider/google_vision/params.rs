use std::fmt;

use serde::{Deserialize, Serialize};

/// Constructor parameters for [`GoogleVisionBackend`].
///
/// [`GoogleVisionBackend`]: super::GoogleVisionBackend
#[derive(Clone, Serialize, Deserialize)]
pub struct GoogleVisionParams {
    /// Google Cloud API key.
    pub api_key: String,
}

impl fmt::Debug for GoogleVisionParams {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("GoogleVisionParams")
            .field("api_key", &"***")
            .finish()
    }
}
