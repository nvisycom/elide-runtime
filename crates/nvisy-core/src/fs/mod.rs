//! Filesystem module for content storage and retrieval
//!
//! This module provides content storage backed by fjall, an embedded
//! key-value store, along with metadata handling and content classification.
//!
//! # Core Types
//!
//! - [`ContentRegistry`]: Key-value store for content data and metadata
//! - [`ContentHandler`]: Lightweight handle to a registered content entry
//! - [`ContentMetadata`]: Metadata information for content files
//! - [`ContentKind`]: Classification of content types by file extension

mod content_handler;
mod content_kind;
mod content_metadata;
mod content_registry;
mod document_type;

pub use content_handler::ContentHandler;
pub use content_kind::ContentKind;
pub use content_metadata::ContentMetadata;
pub use content_registry::ContentRegistry;
pub use document_type::DocumentType;
