//! Typed request bodies and path parameters for API endpoints.
//!
//! Each struct derives [`Deserialize`] and
//! [`JsonSchema`] for automatic OpenAPI schema
//! generation via aide.
//!
//! [`Deserialize`]: serde::Deserialize
//! [`JsonSchema`]: schemars::JsonSchema

mod files;
mod pagination;
mod path;
mod policies;
mod runs;

pub use self::files::NewFile;
pub use self::pagination::{Page, Pagination};
pub use self::path::{ContentPath, PolicyPath, RunPath};
pub use self::policies::NewPolicy;
pub use self::runs::{NewRun, RunQuery};
