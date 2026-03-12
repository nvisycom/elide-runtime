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

pub use self::compression::Compression;
pub use self::conversion::Conversion;
pub use self::decompression::Decompression;
pub use self::decryption::Decryption;
pub use self::encryption::Encryption;
pub use self::export::Export;
pub use self::import::Import;
