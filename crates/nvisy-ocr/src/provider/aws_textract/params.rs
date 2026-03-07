use std::fmt;

use serde::{Deserialize, Serialize};

/// Constructor parameters for [`AwsTextractBackend`].
///
/// [`AwsTextractBackend`]: super::AwsTextractBackend
#[derive(Clone, Serialize, Deserialize)]
pub struct AwsTextractParams {
    /// AWS access key ID.
    pub access_key: String,
    /// AWS secret access key.
    pub secret_key: String,
    /// AWS region (e.g. `us-east-1`).
    pub region: String,
}

impl fmt::Debug for AwsTextractParams {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("AwsTextractParams")
            .field("access_key", &"***")
            .field("secret_key", &"***")
            .field("region", &self.region)
            .finish()
    }
}
