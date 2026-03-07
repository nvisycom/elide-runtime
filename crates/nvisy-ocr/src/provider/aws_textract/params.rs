use std::fmt;

use serde::{Deserialize, Serialize};

/// Constructor parameters for [`AwsTextractBackend`].
///
/// [`AwsTextractBackend`]: super::AwsTextractBackend
#[derive(Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "config", derive(clap::Args))]
pub struct AwsTextractParams {
    /// AWS access key ID.
    #[cfg_attr(
        feature = "config",
        arg(long, env = "AWS_ACCESS_KEY_ID", hide_env_values = true)
    )]
    pub access_key: String,
    /// AWS secret access key.
    #[cfg_attr(
        feature = "config",
        arg(long, env = "AWS_SECRET_ACCESS_KEY", hide_env_values = true)
    )]
    pub secret_key: String,
    /// AWS region (e.g. `us-east-1`).
    #[cfg_attr(feature = "config", arg(long, env = "AWS_REGION"))]
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
