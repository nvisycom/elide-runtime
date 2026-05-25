use nvisy_ontology::document::{Chunk, ChunkMeta, Document, DocumentMeta, Span};
use nvisy_ontology::entity::{Location, TextLocation};

use super::shared::{assert_roundtrip, asserted};

#[test]
fn text_document_round_trips() {
    let text = "Patient: John Doe. Email: john@example.com.";
    let doc = Document {
        meta: DocumentMeta {
            languages: vec![asserted("en")],
            headers: vec![],
        },
        chunks: vec![Chunk {
            text: text.to_owned(),
            spans: vec![Span {
                text_start: 0,
                text_end: text.len(),
                confidence: None,
                source: Location::Text(
                    TextLocation::builder()
                        .with_start_offset(0_usize)
                        .with_end_offset(text.len())
                        .build()
                        .unwrap(),
                ),
            }],
            meta: ChunkMeta::Document,
        }],
    };

    assert_roundtrip(&doc);

    let chunk = &doc.chunks[0];
    let span = chunk.span_at(15).expect("span at offset 15");
    assert_eq!(span.text_start, 0);
    assert_eq!(span.text_end, text.len());
}
