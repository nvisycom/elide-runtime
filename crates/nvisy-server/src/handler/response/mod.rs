//! Typed response bodies for API endpoints.
//!
//! Each struct derives [`Serialize`](serde::Serialize) and
//! [`JsonSchema`](schemars::JsonSchema) for automatic OpenAPI schema
//! generation via aide.

mod check;
mod contexts;
mod error;
mod files;
mod runs;

pub use self::check::{Analytics, Health, ServiceStatus};
pub use self::contexts::{Context, ContextId, ContextList};
pub use self::error::ErrorResponse;
pub use self::files::{File, FileId, FileList};
pub use self::runs::{Run, RunList, RunResult};
