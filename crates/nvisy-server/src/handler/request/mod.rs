//! Typed request bodies and path parameters for API endpoints.
//!
//! Each struct derives [`Deserialize`](serde::Deserialize) and
//! [`JsonSchema`](schemars::JsonSchema) for automatic OpenAPI schema
//! generation via aide.

mod contexts;
mod files;
mod pagination;
mod path;
mod runs;

pub use self::contexts::NewContext;
pub use self::files::NewFile;
pub use self::pagination::{Page, Pagination};
pub use self::path::{ContentPath, ContextPath, RunPath};
pub use self::runs::{NewRun, RunQuery};
