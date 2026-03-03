//! LLM backend: provider connections and usage tracking.

mod http;
mod metrics;
mod provider;

pub use http::HttpClientConfig;
pub use metrics::{UsageStats, UsageTracker};
pub use provider::{AuthenticatedProvider, UnauthenticatedProvider};
pub(crate) use http::build_http_client;
