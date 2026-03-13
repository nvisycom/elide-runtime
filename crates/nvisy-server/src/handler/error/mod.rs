//! HTTP error types for API handlers.
//!
//! Provides [`Error`], [`ErrorKind`], and a [`Result`] alias used as
//! the standard error type across all handler, extractor, and
//! middleware code in the server.

mod from_core;
mod http_error;
mod http_kind;

pub use self::http_error::{Error, Result};
pub use self::http_kind::ErrorKind;
