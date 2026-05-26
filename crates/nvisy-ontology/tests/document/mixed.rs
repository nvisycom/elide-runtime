use nvisy_ontology::document::{Chunk, ChunkMeta, Document, DocumentMeta, Span};
use nvisy_ontology::entity::{ImageLocation, Location, TextLocation};
use nvisy_ontology::primitive::BoundingBox;

use super::shared::assert_roundtrip;

#[test]
fn mixed_chunks_in_one_document() {
    // A "rich" document with two chunks: a native-text page and an
    // OCR'd page in the same document.
    let doc = Document {
        meta: DocumentMeta::default(),
        chunks: vec![
            // Page 1: native text extraction.
            Chunk {
                text: "Header text.".into(),
                spans: vec![Span {
                    text_start: 0,
                    text_end: 12,
                    confidence: None,
                    source: Location::Text(TextLocation::new(0, 12)),
                }],
                meta: ChunkMeta::Page {
                    number: 1,
                    width: None,
                    height: None,
                },
            },
            // Page 2: OCR'd image.
            Chunk {
                text: "OCR'd text".into(),
                spans: vec![Span {
                    text_start: 0,
                    text_end: 10,
                    confidence: Some(0.88),
                    source: Location::Image(
                        ImageLocation::builder()
                            .with_bounding_box(BoundingBox {
                                x: 0.0,
                                y: 0.0,
                                width: 100.0,
                                height: 20.0,
                            })
                            .with_page_number(2_u32)
                            .build()
                            .unwrap(),
                    ),
                }],
                meta: ChunkMeta::Page {
                    number: 2,
                    width: Some(800.0),
                    height: Some(600.0),
                },
            },
        ],
    };

    assert_roundtrip(&doc);
    assert_eq!(doc.chunks.len(), 2);
    // Same enum, different variants, dispatched uniformly:
    let kinds: Vec<&str> = doc
        .spans()
        .map(|(_, s)| match &s.source {
            Location::Text(_) => "text",
            Location::Image(_) => "image",
            Location::Audio(_) => "audio",
            Location::Tabular(_) => "tabular",
            _ => "other",
        })
        .collect();
    assert_eq!(kinds, vec!["text", "image"]);
}
