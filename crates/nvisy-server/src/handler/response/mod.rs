//! Typed response bodies for API endpoints.
//!
//! Each struct derives [`Serialize`] and [`JsonSchema`] for
//! automatic OpenAPI schema generation via aide.
//!
//! Per-run wrappers (the per-modality entity records, the
//! [`runs::RunResponse`] assembler, etc.) live in
//! [`runs`] — a public submodule so the CLI and other
//! external consumers can name them on their own request /
//! response types without going through `pub use`.
//!
//! [`Serialize`]: serde::Serialize
//! [`JsonSchema`]: schemars::JsonSchema

mod check;
mod contexts;
mod detections;
mod error;
mod files;
mod page;
mod policies;
mod redactions;
pub mod runs;

pub use self::check::Health;
pub use self::contexts::ContextSummary;
pub use self::detections::DetectionId;
pub use self::error::ErrorResponse;
pub use self::files::{FileId, FileMetadataResponse};
pub use self::page::Page;
pub use self::policies::PolicySummary;
pub use self::redactions::{RedactionOutput, RedactionResult};
pub use self::runs::RunResponse;
