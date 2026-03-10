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
//! | [`Encryption`]  | Encrypts content at rest or in transit                 |
//! | [`Publish`]     | Delivers redacted content to a downstream target       |
//!
//! [`Document`]: nvisy_codec::Document

mod compression;
mod conversion;
mod encryption;
mod ingestion;
mod publish;

pub use compression::Compression;
pub use conversion::Conversion;
pub use encryption::Encryption;
pub use ingestion::Ingestion;
pub use publish::Publish;
