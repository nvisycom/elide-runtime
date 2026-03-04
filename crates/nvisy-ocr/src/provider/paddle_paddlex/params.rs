use serde::{Deserialize, Serialize};

/// Constructor parameters for [`PaddleXBackend`].
///
/// [`PaddleXBackend`]: super::PaddleXBackend
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PaddleXParams {
    /// Base URL of the PaddleX server.
    pub base_url: String,
}
