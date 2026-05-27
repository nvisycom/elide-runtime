//! In-memory decode helpers for use in unit and integration tests.
//!
//! Enabled by the `test-utils` feature, and always available within
//! this crate's own test builds. Each helper constructs a synthetic
//! [`Content`] with a hardcoded MIME type, runs it through
//! [`crate::decode`], and returns a ready-to-wrap [`DocumentHandle`].
//! Use them when you want a real codec handle without going through
//! the full importer (decompression, decryption, registry I/O).

use nvisy_codec::DocumentHandle;
use nvisy_core::Error;
use nvisy_core::content::{Content, ContentData, ContentMetadata};

/// Decode `text` as a `text/plain` document and return the handle.
///
/// Panics if decoding fails — these helpers are for tests only.
#[cfg(feature = "txt")]
pub async fn decode_text(text: &str) -> Result<DocumentHandle, Error> {
    let data = ContentData::from(text.to_owned());
    let meta = ContentMetadata::new().with_content_type("text/plain");
    let content = Content::with_metadata(data, meta);
    crate::decode(&content).await
}
