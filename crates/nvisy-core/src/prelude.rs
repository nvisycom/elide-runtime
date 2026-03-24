//! Convenience re-exports for common nvisy-core types.
//!
//! Import everything from this module to get the most commonly used
//! types without individual `use` statements.

pub use crate::content::{Content, ContentData, ContentMetadata, ContentSource, DataReference};
pub use crate::media::ContentKind;
pub use crate::{Error, ErrorKind, Result};
