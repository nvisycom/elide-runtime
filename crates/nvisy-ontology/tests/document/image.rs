use nvisy_ontology::document::{Chunk, ChunkMeta, Document, DocumentMeta, Span};
use nvisy_ontology::entity::{ImageLocation, Location};
use nvisy_ontology::primitive::{BoundingBox, Polygon, Vertex};

use super::shared::assert_roundtrip;

#[test]
fn ocr_page_round_trips_with_polygon_per_word() {
    let chunk_text = "John Smith";
    let words = [
        ("John", 0, 4, BoundingBox { x: 10.0, y: 20.0, width: 50.0, height: 18.0 }),
        ("Smith", 5, 10, BoundingBox { x: 65.0, y: 20.0, width: 60.0, height: 18.0 }),
    ];

    let mut spans = Vec::new();
    for (_, start, end, bbox) in &words {
        let polygon = Polygon {
            vertices: vec![
                Vertex::new(bbox.x, bbox.y),
                Vertex::new(bbox.x + bbox.width, bbox.y),
                Vertex::new(bbox.x + bbox.width, bbox.y + bbox.height),
                Vertex::new(bbox.x, bbox.y + bbox.height),
            ],
        };
        let location = ImageLocation::builder()
            .with_bounding_box(*bbox)
            .with_polygon(polygon)
            .with_page_number(1_u32)
            .build()
            .unwrap();
        spans.push(Span {
            text_start: *start,
            text_end: *end,
            confidence: Some(0.97),
            source: Location::Image(location),
        });
    }

    let doc = Document {
        meta: DocumentMeta::default(),
        chunks: vec![Chunk {
            text: chunk_text.to_owned(),
            spans,
            meta: ChunkMeta::Page {
                number: 1,
                width: Some(1024.0),
                height: Some(768.0),
            },
        }],
    };

    assert_roundtrip(&doc);

    // "John" covers 0..4; offset 2 (inside "John") should hit it.
    let chunk = &doc.chunks[0];
    let span = chunk.span_at(2).expect("span at offset 2");
    let Location::Image(loc) = &span.source else {
        panic!("expected Image location");
    };
    assert_eq!(loc.bounding_box.x, 10.0);
    assert!(loc.polygon.is_some());

    // Range covering both words returns both spans.
    let hits: Vec<_> = chunk.spans_in(0..10).collect();
    assert_eq!(hits.len(), 2);
}
