//! Filesystem module for content file operations
//!
//! This module provides filesystem-specific functionality for working with
//! content files, including file metadata handling and archive operations.
//!
//! # Core Types
//!
//! - [`ContentFile`]: A file wrapper that combines filesystem operations with content tracking
//! - [`ContentMetadata`]: Metadata information for content files
//! - [`ContentKind`]: Classification of content types by file extension
//!
//! # Example
//!
//! ```no_run
//! use nvisy_core::fs::ContentFile;
//! use nvisy_core::io::ContentData;
//!
//! async fn example() -> Result<(), Box<dyn std::error::Error>> {
//!     // Create a new file
//!     let mut content_file = ContentFile::create("example.txt").await?;
//!
//!     // Write some content
//!     let content_data = ContentData::from("Hello, world!");
//!     let metadata = content_file.write_from_content_data(content_data).await?;
//!
//!     println!("Written to: {:?}", metadata.source_path);
//!     Ok(())
//! }
//! ```

mod content_file;
mod content_handler;
mod content_kind;
mod content_metadata;
mod content_registry;
mod document_type;

// Re-export main types
pub use content_file::ContentFile;
pub use content_handler::ContentHandler;
pub use content_kind::ContentKind;
pub use content_metadata::ContentMetadata;
pub use content_registry::ContentRegistry;
pub use document_type::DocumentType;
