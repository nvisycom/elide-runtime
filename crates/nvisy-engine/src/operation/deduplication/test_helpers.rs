//! Test helpers for deduplication tests.

use nvisy_codec::ContentHandle;
use nvisy_core::content::{Content, ContentData, ContentMetadata, ContentSource};
use nvisy_ontology::entity::{
    Entity, EntityCategory, EntityKind, Location, RecognitionMethod, TextLocation,
};

use crate::operation::Document;

/// Create a test [`Document`] from plain text content.
pub(super) async fn text_document(text: &str) -> Document {
    let data = ContentData::from_text(ContentSource::new(), text);
    let meta = ContentMetadata::new().with_content_type("text/plain");
    let content = Content::with_metadata(data, meta.clone());
    let handle = ContentHandle::decode(&content).await.expect("decode text");
    Document::new(handle, meta)
}

/// Build a text entity at the given byte offsets for testing.
pub(super) fn text_entity(
    value: &str,
    method: RecognitionMethod,
    confidence: f64,
    start: usize,
    end: usize,
) -> Entity {
    Entity::builder()
        .with_category(EntityCategory::PersonalIdentity)
        .with_entity_kind(EntityKind::PersonName)
        .with_recognition_methods(vec![method])
        .with_confidence(confidence)
        .with_location(Location::from(
            TextLocation::builder()
                .with_text(value)
                .with_start_offset(start)
                .with_end_offset(end)
                .build()
                .unwrap(),
        ))
        .build()
        .unwrap()
}
