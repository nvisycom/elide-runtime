//! Typed request bodies for API endpoints.
//!
//! Each struct derives [`Deserialize`](serde::Deserialize) and
//! [`JsonSchema`](schemars::JsonSchema) for automatic OpenAPI schema
//! generation via aide.

mod execute;
mod redact;

pub use execute::ExecuteRequest;
pub use redact::RedactionRequest;
