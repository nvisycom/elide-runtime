//! HTTP error types for API handlers.
//!
//! Provides [`Error`], [`ErrorKind`], and a [`Result`] alias used as
//! the standard error type across all handler, extractor, and
//! middleware code in the server.

mod http_error;

pub use http_error::{Error, ErrorKind, Result};
