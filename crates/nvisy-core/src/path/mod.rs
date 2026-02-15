//! Path module for content source identification
//!
//! This module provides functionality for uniquely identifying content sources
//! throughout the nvisy system using UUIDv7-based identifiers.

mod source;

// Re-export core types
pub use source::ContentSource;
