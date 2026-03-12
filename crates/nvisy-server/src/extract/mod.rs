//! Custom extractors for axum handlers.

mod json;
mod path;
mod version;

pub use self::json::Json;
pub use self::path::Path;
pub use self::version::Version;
