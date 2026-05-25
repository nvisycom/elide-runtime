use nvisy_ontology::document::{Chunk, ChunkMeta, ColumnHeader, Document, DocumentMeta, Span};
use nvisy_ontology::entity::{Location, TabularLocation};

use super::shared::assert_roundtrip;

#[test]
fn tabular_round_trips_with_one_chunk_per_row_and_headers_at_doc_level() {
    // Row 0: "Alice,30,alice@example.com"
    let cells: [(&str, usize, usize, u32); 3] = [
        ("Alice", 0, 5, 0),
        ("30", 6, 8, 1),
        ("alice@example.com", 9, 26, 2),
    ];

    let mut spans = Vec::new();
    let mut row_text = String::new();
    for (i, (cell_text, start, end, col)) in cells.iter().enumerate() {
        if i > 0 {
            row_text.push(',');
        }
        row_text.push_str(cell_text);
        let location = TabularLocation::builder()
            .with_row_index(0_usize)
            .with_column_index(*col as usize)
            .with_start_offset(0_usize)
            .with_end_offset(cell_text.len())
            .build()
            .unwrap();
        spans.push(Span {
            text_start: *start,
            text_end: *end,
            confidence: None,
            source: Location::Tabular(location),
        });
    }

    let doc = Document {
        meta: DocumentMeta {
            languages: vec![],
            headers: vec![
                ColumnHeader { column_index: 0, text: "name".into() },
                ColumnHeader { column_index: 1, text: "age".into() },
                ColumnHeader { column_index: 2, text: "email".into() },
            ],
        },
        chunks: vec![Chunk {
            text: row_text,
            spans,
            meta: ChunkMeta::Row { index: 0 },
        }],
    };

    assert_roundtrip(&doc);

    // Offset 12 falls inside "alice@example.com" (starts at 9).
    let chunk = &doc.chunks[0];
    let span = chunk.span_at(12).expect("span at offset 12");
    let Location::Tabular(loc) = &span.source else {
        panic!("expected Tabular");
    };
    assert_eq!(loc.column_index, 2);
}
