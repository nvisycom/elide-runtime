//! Typed request bodies for API endpoints.
//!
//! Each struct derives [`Deserialize`](serde::Deserialize) and
//! [`JsonSchema`](schemars::JsonSchema) for automatic OpenAPI schema
//! generation via aide.

mod base64;
mod contexts;
mod files;
mod path;
mod process;

pub use self::base64::Base64;
pub use contexts::ContextUpload;
pub use files::FileUpload;
pub use path::{ActorQuery, ContentPath, ContextPath};
pub use process::ProcessRequest;
