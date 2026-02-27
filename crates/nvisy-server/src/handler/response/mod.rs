//! Typed response bodies and error types for API endpoints.
//!
//! Each struct derives [`Serialize`](serde::Serialize) and
//! [`JsonSchema`](schemars::JsonSchema) for automatic OpenAPI schema
//! generation via aide. [`ServerError`] maps
//! [`nvisy_core::ErrorKind`] to HTTP status codes and produces a
//! JSON error body.

mod check;
pub mod error;
mod execute;
mod ingest;
mod redact;

pub use check::{Analytics, Health};
pub use error::ServerError;
pub use execute::ExecuteResponse;
pub use ingest::{DeleteAllResponse, DeleteResponse, DownloadResponse, UploadResponse};
pub use redact::RedactionResponse;
