use serde::{Deserialize, Serialize};

/// Constructor parameters for [`DoctrBackend`].
///
/// [`DoctrBackend`]: super::DoctrBackend
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "config", derive(clap::Args))]
pub struct DoctrParams {
    /// Base URL of the DocTR server.
    #[cfg_attr(feature = "config", arg(long, env = "DOCTR_BASE_URL"))]
    pub base_url: String,
}
