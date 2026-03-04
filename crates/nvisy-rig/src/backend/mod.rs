//! LLM backend: provider connections and usage tracking.

mod http;
mod metrics;
mod provider;

pub use http::{HttpConfig, build_http_client};
pub use metrics::{UsageStats, UsageTracker};
pub use provider::{AuthenticatedProvider, UnauthenticatedProvider};
