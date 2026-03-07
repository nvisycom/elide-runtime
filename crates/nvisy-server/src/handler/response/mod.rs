//! Typed response bodies and error types for API endpoints.
//!
//! Each struct derives [`Serialize`](serde::Serialize) and
//! [`JsonSchema`](schemars::JsonSchema) for automatic OpenAPI schema
//! generation via aide. [`ErrorResponse`] is the serializable JSON
//! body returned by every error path.

mod check;
mod contexts;
mod error;
mod files;
mod process;

pub use check::{Analytics, Health, ServiceStatus};
pub use contexts::{Context, ContextId, ContextList};
pub use error::ErrorResponse;
pub use files::{File, FileId, FileList};
pub use process::ProcessResult;
