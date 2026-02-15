//! I/O module for content handling and processing
//!
//! This module provides the core I/O abstractions for handling content data,
//! including content data structures and async read/write traits.
//!
//! # Core Types
//!
//! - [`ContentData`]: Container for content data with metadata, hashing, and size utilities
//!
//! # Traits
//!
//! - [`AsyncContentRead`]: Async trait for reading content from various sources
//! - [`AsyncContentWrite`]: Async trait for writing content to various destinations

mod content;
mod content_data;
mod content_read;
mod content_write;
mod data_reference;

// Re-export core types and traits
pub use content::Content;
pub use content_data::{ContentBytes, ContentData};
pub use content_read::AsyncContentRead;
pub use content_write::AsyncContentWrite;
pub use data_reference::DataReference;
