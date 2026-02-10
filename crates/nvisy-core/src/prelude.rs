//! Convenience re-exports for common nvisy-core types.
//!
//! Import everything from this module to get the most commonly used
//! types without individual `use` statements.

pub use crate::datatypes::blob::Blob;
pub use crate::datatypes::Data;
pub use crate::error::{Error, ErrorKind, Result};
pub use crate::registry::action::Action;
pub use crate::registry::loader::Loader;
pub use crate::registry::provider::{ConnectedInstance, ProviderFactory};
pub use crate::registry::stream::{StreamSource, StreamTarget};
pub use crate::ontology::entity::{DetectionMethod, EntityCategory};
pub use crate::ontology::redaction::RedactionMethod;
