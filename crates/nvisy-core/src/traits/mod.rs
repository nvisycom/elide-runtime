//! Core traits defining the pipeline extension points.
//!
//! Actions, loaders, stream sources/targets, and provider factories
//! are the primary interfaces that plugins implement.

pub mod action;
pub mod loader;
pub mod provider;
pub mod stream;
