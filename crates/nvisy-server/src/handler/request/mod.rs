//! Typed request bodies for API endpoints.
//!
//! Each struct derives [`Deserialize`](serde::Deserialize) and
//! [`JsonSchema`](schemars::JsonSchema) for automatic OpenAPI schema
//! generation via aide.

mod contexts;
mod files;
mod path;
mod process;

pub use contexts::ContextUpload;
pub use files::FileUpload;
pub use path::ContentPath;
pub use process::ProcessRequest;
