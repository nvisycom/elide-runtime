use serde::{Deserialize, Serialize};

/// Constructor parameters for [`DoctrBackend`].
///
/// [`DoctrBackend`]: super::DoctrBackend
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DoctrParams {
    /// Base URL of the DocTR server.
    pub base_url: String,
}
