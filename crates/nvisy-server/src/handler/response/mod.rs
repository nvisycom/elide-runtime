//! Typed response bodies and error types for API endpoints.

mod check;
pub mod error;
mod execute;
mod ingest;
mod redact;

pub use check::Analytics;
pub use error::ServerError;
pub use execute::ExecuteResponse;
pub use ingest::{DownloadResponse, UploadResponse};
pub use redact::RedactionResponse;
