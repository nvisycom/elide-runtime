//! Custom extractors for axum handlers.

mod json;
mod path;
mod version;

pub use json::Json;
pub use path::Path;
pub use version::Version;
