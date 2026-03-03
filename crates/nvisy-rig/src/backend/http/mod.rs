//! Shared HTTP client with timeout, retry, and tracing middleware.

mod client;
mod config;

pub use config::HttpClientConfig;
pub(crate) use client::build_http_client;
