//! Round-trip tests for [`Document`] across every supported modality.
//!
//! For each modality (text, image OCR, audio transcription, tabular,
//! and mixed-modality), the test:
//!
//! 1. Builds a `Document` with realistic spans for that modality.
//! 2. Serializes to JSON, deserializes back, asserts equality.
//! 3. Exercises `Chunk::span_at` / `Chunk::spans_in` to confirm
//!    the source-mapping lookup behaves correctly.
//!
//! [`Document`]: nvisy_ontology::document::Document

#[path = "document/shared.rs"]
mod shared;

#[path = "document/audio.rs"]
mod audio;
#[path = "document/image.rs"]
mod image;
#[path = "document/mixed.rs"]
mod mixed;
#[path = "document/tabular.rs"]
mod tabular;
#[path = "document/text.rs"]
mod text;
