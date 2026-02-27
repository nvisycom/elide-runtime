//! Typed response bodies and error types for API endpoints.
//!
//! Each struct derives [`Serialize`](serde::Serialize) and
//! [`JsonSchema`](schemars::JsonSchema) for automatic OpenAPI schema
//! generation via aide. [`ErrorResponse`] is the serializable JSON
//! body returned by every error path.

mod check;
pub mod error;
mod execute;
mod ingest;
mod redact;

pub use check::{Analytics, Health};
pub use error::ErrorResponse;
pub use execute::ExecuteResponse;
pub use ingest::{DeleteAllResponse, DeleteResponse, DownloadResponse, UploadResponse};
pub use redact::RedactionResponse;
