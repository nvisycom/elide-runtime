//! Lifecycle operations: file I/O and context management.
//!
//! | Operation          | Description                                           |
//! |--------------------|-------------------------------------------------------|
//! | [`ImportFile`]     | Decodes raw bytes into a typed [`Document`]           |
//! | [`ExportFile`]     | Delivers redacted content to a downstream target      |
//! | [`LoadContext`]    | Loads contexts from the registry into the envelope    |
//! | [`SaveContext`]    | Persists selected envelope contexts to the registry   |
//! | [`GenerateContext`]| Generates contexts from pipeline results (stub)       |
//!
//! Compression and encryption are utility steps within [`ImportFile`]
//! and [`ExportFile`], not standalone operations. See
//! [`compression`](crate::operation::utility::compression) and
//! [`encryption`](crate::operation::utility::encryption).
//!
//! [`Document`]: nvisy_codec::Document

mod export_file;
mod generate_context;
mod import_file;
mod load_context;
mod save_context;

pub use self::export_file::ExportFile;
pub use self::generate_context::GenerateContext;
pub use self::import_file::ImportFile;
pub use self::load_context::LoadContext;
pub use self::save_context::SaveContext;
