use serde::{Deserialize, Serialize};

/// Constructor parameters for [`PaddleXBackend`].
///
/// [`PaddleXBackend`]: super::PaddleXBackend
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "config", derive(clap::Args))]
pub struct PaddleXParams {
    /// Base URL of the PaddleX server.
    #[cfg_attr(feature = "config", arg(long, env = "PADDLEX_BASE_URL"))]
    pub base_url: String,
}
