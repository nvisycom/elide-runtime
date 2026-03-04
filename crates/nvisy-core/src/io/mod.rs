//! I/O module for content handling and processing
//!
//! This module provides the core I/O abstractions for handling content data.
//!
//! # Core Types
//!
//! - [`ContentData`]: Container for content data with metadata, hashing, and size utilities
//! - [`ContentBytes`]: Wrapper around `Bytes` for content storage

mod content;
mod content_bytes;
mod content_data;
mod data_reference;
mod encoding;

pub use content::Content;
pub use content_bytes::ContentBytes;
pub use content_data::ContentData;
pub use data_reference::DataReference;
pub use encoding::TextEncoding;
