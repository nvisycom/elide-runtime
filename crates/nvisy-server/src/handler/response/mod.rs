//! Typed response bodies for API endpoints.
//!
//! Each struct derives [`Serialize`](serde::Serialize) and
//! [`JsonSchema`](schemars::JsonSchema) for automatic OpenAPI schema
//! generation via aide.

mod check;
mod contexts;
mod error;
mod files;
mod policies;
mod runs;

pub use self::check::{ComponentCheck, Health, ServiceStatus};
pub use self::contexts::{ContextEntry, ContextId, ContextList};
pub use self::error::ErrorResponse;
pub use self::files::{File, FileEntry, FileId, FileList};
pub use self::policies::{PolicyEntry, PolicyId, PolicyList};
pub use self::runs::RunList;
