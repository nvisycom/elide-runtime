use nvisy_ontology::document::{Chunk, ChunkMeta, Document, DocumentMeta, Span};
use nvisy_ontology::entity::{AudioLocation, Location};
use nvisy_ontology::primitive::TimeSpan;

use super::shared::{assert_roundtrip, asserted};

#[test]
fn audio_transcription_round_trips_as_one_chunk_per_segment() {
    let segments = [
        (
            "Hello there.",
            TimeSpan::new(0, 1_500_000),
            Some("speaker_1"),
        ),
        (
            "General Kenobi.",
            TimeSpan::new(1_500_000, 3_200_000),
            Some("speaker_2"),
        ),
    ];

    let chunks: Vec<Chunk> = segments
        .iter()
        .map(|(text, ts, sp)| {
            let location = AudioLocation::builder()
                .with_time_span(*ts)
                .with_speaker_id(sp.unwrap().to_string())
                .build()
                .unwrap();
            Chunk {
                text: (*text).to_owned(),
                spans: vec![Span {
                    text_start: 0,
                    text_end: text.len(),
                    confidence: Some(0.92),
                    source: Location::Audio(location),
                }],
                meta: ChunkMeta::AudioSegment {
                    time_span: *ts,
                    speaker_id: sp.map(str::to_string),
                },
            }
        })
        .collect();

    let doc = Document {
        meta: DocumentMeta {
            languages: vec![asserted("en")],
            headers: vec![],
        },
        chunks,
    };

    assert_roundtrip(&doc);
    assert_eq!(doc.spans().count(), 2);
}
