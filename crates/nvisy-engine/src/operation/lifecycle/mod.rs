//! Lifecycle operations: content import, packaging, and delivery.
//!
//! These operations manage the content lifecycle from raw bytes through
//! to final delivery, bookending the detection and redaction stages.
//!
//! | Operation         | Description                                           |
//! |-------------------|-------------------------------------------------------|
//! | [`Import`]        | Decodes raw bytes into a typed [`Document`]           |
//! | [`Conversion`]    | Converts content between formats                      |
//! | [`Compression`]   | Compresses content for storage or transfer             |
//! | [`Decompression`] | Decompresses content from storage or transfer          |
//! | [`Encryption`]    | Encrypts content using AES-256-GCM                    |
//! | [`Decryption`]    | Decrypts content back to [`ContentData`]              |
//! | [`Export`]        | Delivers redacted content to a downstream target       |
//!
//! [`Document`]: nvisy_codec::Document
//! [`ContentData`]: nvisy_core::content::ContentData

mod compression;
mod conversion;
mod decompression;
mod decryption;
mod encryption;
mod export;
mod import;

pub use compression::Compression;
pub use conversion::Conversion;
pub use decompression::Decompression;
pub use decryption::Decryption;
pub use encryption::Encryption;
pub use export::Export;
pub use import::Import;
