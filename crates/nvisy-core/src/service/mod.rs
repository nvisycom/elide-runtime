//! Service-level concerns: the runtime's error vocabulary and the
//! healthcheck composition trait. Distinct from the elide toolkit's
//! own error type. Runtime adds request-scoped context and surface
//! categories the toolkit doesn't model.

pub mod error;
pub mod health;

pub use self::error::{Error, ErrorKind, Result};
