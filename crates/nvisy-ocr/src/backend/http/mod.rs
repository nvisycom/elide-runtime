//! Shared HTTP client with timeout, retry, and tracing middleware.

mod client;
mod config;

pub use client::build_http_client;
pub use config::HttpConfig;
