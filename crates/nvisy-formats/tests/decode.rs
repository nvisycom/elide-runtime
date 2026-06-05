//! End-to-end test for the codec registry's MIME-driven decode path.
//!
//! Builds a `ContentData` for each default-feature format and asserts
//! that the registry routes it through the right loader and produces
//! the matching [`UntypedDocumentHandle`] variant.

#![cfg(all(feature = "txt", feature = "json", feature = "csv"))]

use nvisy_codec::{CodecRegistry, UntypedDocumentHandle};
use nvisy_core::content::{ContentData, ContentSource};
use nvisy_formats::CodecRegistryExt;

fn data(body: &[u8]) -> ContentData {
    ContentData::new(ContentSource::new(), body.to_vec().into())
}

async fn decode_by_mime(mime: &str, body: &[u8]) -> UntypedDocumentHandle {
    let registry = CodecRegistry::builtins();
    let format = registry
        .by_content_type(mime)
        .unwrap_or_else(|| panic!("no codec registered for `{mime}`"));
    format
        .loader
        .decode(data(body))
        .await
        .expect("decode succeeds")
}

#[tokio::test]
async fn decode_routes_txt_to_text_modality() {
    let handle = decode_by_mime("text/plain", b"hello\nworld\n").await;
    assert!(matches!(handle, UntypedDocumentHandle::Text(_)));
}

#[tokio::test]
async fn decode_routes_json_to_text_modality() {
    let handle = decode_by_mime("application/json", br#"{"k":"v"}"#).await;
    assert!(matches!(handle, UntypedDocumentHandle::Text(_)));
}

#[tokio::test]
async fn decode_routes_csv_to_tabular_modality() {
    let handle = decode_by_mime("text/csv", b"a,b,c\n1,2,3\n").await;
    assert!(matches!(handle, UntypedDocumentHandle::Tabular(_)));
}

#[tokio::test]
async fn decode_fails_for_unknown_mime() {
    let registry = CodecRegistry::builtins();
    assert!(registry.by_content_type("application/x-unknown").is_none());
}
