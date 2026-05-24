//! End-to-end test for the `decode` entry point.
//!
//! Builds a `Content` for each default-feature format and asserts
//! that `nvisy_formats::decode` routes it to the right modality and
//! that basic accessors work on the returned handle.

#![cfg(all(feature = "txt", feature = "json", feature = "csv"))]

use nvisy_codec::ContentHandle;
use nvisy_core::content::{Content, ContentData, ContentMetadata, ContentSource};

fn content_with(body: &[u8], mime: &str) -> Content {
    let data = ContentData::new(ContentSource::new(), body.to_vec().into());
    let meta = ContentMetadata::new().with_content_type(mime);
    Content::with_metadata(data, meta)
}

#[tokio::test]
async fn decode_routes_txt_to_text_modality() {
    let content = content_with(b"hello\nworld\n", "text/plain");
    let handle = nvisy_formats::decode(&content).await.expect("decode");
    assert!(matches!(handle, ContentHandle::Text(_)));
}

#[tokio::test]
async fn decode_routes_json_to_text_modality() {
    let content = content_with(br#"{"k":"v"}"#, "application/json");
    let handle = nvisy_formats::decode(&content).await.expect("decode");
    assert!(matches!(handle, ContentHandle::Text(_)));
}

#[tokio::test]
async fn decode_routes_csv_to_tabular_modality() {
    let content = content_with(b"a,b,c\n1,2,3\n", "text/csv");
    let handle = nvisy_formats::decode(&content).await.expect("decode");
    assert!(matches!(handle, ContentHandle::Tabular(_)));
}

#[tokio::test]
async fn decode_fails_without_mime() {
    let data = ContentData::new(ContentSource::new(), b"???".to_vec().into());
    let content = Content::with_metadata(data, ContentMetadata::new());
    let err = nvisy_formats::decode(&content).await.unwrap_err();
    let msg = format!("{err}");
    assert!(msg.contains("unable to detect"), "got: {msg}");
}
