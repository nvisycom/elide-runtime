//! Typed response bodies for API endpoints.
//!
//! Each struct derives [`Serialize`] and
//! [`JsonSchema`] for automatic OpenAPI schema
//! generation via aide.
//!
//! [`Serialize`]: serde::Serialize
//! [`JsonSchema`]: schemars::JsonSchema

mod check;
mod detections;
mod error;
mod files;
mod page;
mod policies;
mod redactions;

pub use self::check::{ComponentCheck, Health, ServiceStatus};
pub use self::detections::{DetectionId, DetectionList};
pub use self::error::ErrorResponse;
pub use self::files::{FileEntry, FileId, FileList};
pub use self::page::Page;
pub use self::policies::{PolicyEntry, PolicyId, PolicyList};
pub use self::redactions::{RedactionId, RedactionList};
