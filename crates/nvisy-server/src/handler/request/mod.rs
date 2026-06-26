//! Typed request bodies and path parameters for API endpoints.
//!
//! Each struct derives [`Deserialize`] and [`JsonSchema`] for
//! automatic OpenAPI schema generation via aide.
//!
//! [`Deserialize`]: serde::Deserialize
//! [`JsonSchema`]: schemars::JsonSchema

pub mod analyzer;
mod contexts;
mod detections;
mod files;
mod pagination;
mod path;
mod policies;
mod redactions;
mod refs;

pub use self::contexts::NewContext;
pub use self::detections::{DetectionQuery, NewDetection};
pub use self::files::FileQuery;
pub use self::pagination::{MAX_PAGE_LIMIT, Pagination};
pub use self::path::{
    ContextIdPath, ContextVersionPath, DetectionPath, FilePath, PolicyIdPath, PolicyVersionPath,
    RedactionPath,
};
pub use self::policies::NewPolicy;
pub use self::redactions::NewRedaction;
