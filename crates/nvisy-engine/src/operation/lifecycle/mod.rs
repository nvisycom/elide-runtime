//! Lifecycle operations: content ingestion, packaging, and delivery.
//!
//! These operations manage the content lifecycle from raw bytes through
//! to final delivery, bookending the detection and redaction stages.
//!
//! | Operation       | Description                                           |
//! |-----------------|-------------------------------------------------------|
//! | [`Ingestion`]   | Decodes raw bytes into a typed [`Document`]           |
//! | [`Conversion`]  | Converts content between formats                      |
//! | [`Compression`] | Compresses content for storage or transfer             |
//! | [`Encryption`]  | Encrypts content using AES-256-GCM                    |
//! | [`Decryption`]  | Decrypts content back to [`ContentData`]              |
//! | [`Publish`]     | Delivers redacted content to a downstream target       |
//!
//! [`Document`]: nvisy_codec::Document
//! [`ContentData`]: nvisy_core::content::ContentData

mod compression;
mod conversion;
mod decryption;
mod encryption;
mod ingestion;
mod publish;

pub use compression::Compression;
pub use conversion::Conversion;
pub use decryption::Decryption;
pub use encryption::Encryption;
pub use ingestion::Ingestion;
pub use publish::Publish;
