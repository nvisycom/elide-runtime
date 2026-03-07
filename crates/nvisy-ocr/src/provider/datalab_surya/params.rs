use serde::{Deserialize, Serialize};

/// Constructor parameters for [`SuryaBackend`].
///
/// [`SuryaBackend`]: super::SuryaBackend
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "config", derive(clap::Args))]
pub struct SuryaParams {
    /// Base URL of the Surya server.
    #[cfg_attr(feature = "config", arg(long, env = "SURYA_BASE_URL"))]
    pub base_url: String,
}
