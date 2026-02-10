//! Format-specific blob loaders.
//!
//! Each loader converts raw [`Blob`](nvisy_core::datatypes::blob::Blob) bytes
//! into one or more [`Document`](nvisy_core::datatypes::document::Document)s
//! that downstream actions can process.

/// Loader for CSV files.
pub mod csv_loader;
/// Loader for JSON files.
pub mod json_loader;
/// Loader for plain-text files.
pub mod plaintext;
