//! LLM backend: provider connections and usage tracking.

mod http_client;
mod metrics;
mod provider;

pub use http_client::HttpClientConfig;
pub use metrics::{UsageStats, UsageTracker};
pub use provider::{AuthenticatedProvider, UnauthenticatedProvider};
pub(crate) use http_client::build_http_client;
