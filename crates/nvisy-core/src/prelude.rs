//! Convenience re-exports for common nvisy-core types.
//!
//! Import everything from this module to get the most commonly used
//! types without individual `use` statements.

pub use crate::{Error, ErrorKind, Result};
pub use crate::fs::{ContentHandler, ContentKind, ContentMetadata, ContentRegistry};
pub use crate::io::{Content, ContentBytes, ContentData, DataReference};
pub use crate::path::ContentSource;
