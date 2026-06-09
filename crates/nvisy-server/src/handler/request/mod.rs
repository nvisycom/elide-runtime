//! Typed request bodies and path parameters for API endpoints.
//!
//! Each struct derives [`Deserialize`] and [`JsonSchema`] for
//! automatic OpenAPI schema generation via aide.
//!
//! [`Deserialize`]: serde::Deserialize
//! [`JsonSchema`]: schemars::JsonSchema

mod detections;
mod files;
mod pagination;
mod path;
mod policies;
mod redactions;

pub use self::detections::{DetectionQuery, NewDetection};
pub use self::files::NewFile;
pub use self::pagination::{MAX_PAGE_LIMIT, Pagination};
pub use self::path::{ContentPath, DetectionPath, PolicyPath, RedactionPath};
pub use self::policies::NewPolicy;
pub use self::redactions::{NewRedaction, RedactionQuery};
