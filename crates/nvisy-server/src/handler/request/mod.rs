//! Typed request bodies for API endpoints.
//!
//! Each struct derives [`Deserialize`](serde::Deserialize) and
//! [`JsonSchema`](schemars::JsonSchema) for automatic OpenAPI schema
//! generation via aide.

mod contexts;
mod files;
mod path;
mod process;

pub use self::contexts::NewContext;
pub use self::files::NewFile;
pub use self::path::{ActorQuery, ContentPath, ContextPath};
pub use self::process::NewProcess;
