//! Shared helpers for the per-modality document round-trip tests.

use nvisy_ontology::document::Document;
use nvisy_ontology::primitive::{LanguageDetection, LanguageProvenance, LanguageTag};

/// Serialize to JSON, deserialize back, assert equality.
pub fn assert_roundtrip(doc: &Document) {
    let json = serde_json::to_string(doc).expect("serialize");
    let back: Document = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(doc, &back, "round-trip mismatch:\n{json}");
}

/// Build a caller-asserted single-language [`LanguageDetection`]
/// from a BCP-47 string — convenience for fixtures.
pub fn asserted(tag: &str) -> LanguageDetection {
    let language: LanguageTag = tag.parse().expect("valid BCP-47 tag");
    LanguageDetection {
        language,
        confidence: None,
        provenance: LanguageProvenance::Asserted,
        span: None,
    }
}
