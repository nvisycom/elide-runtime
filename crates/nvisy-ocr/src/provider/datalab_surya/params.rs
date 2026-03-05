use serde::{Deserialize, Serialize};

/// Constructor parameters for [`SuryaBackend`].
///
/// [`SuryaBackend`]: super::SuryaBackend
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SuryaParams {
    /// Base URL of the Surya server.
    pub base_url: String,
}
