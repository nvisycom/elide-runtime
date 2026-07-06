//! Attach the lingua language-detection [`Enricher<Text>`].
//!
//! Text-modality only — writes the detected language into the
//! per-request recognizer context, so pattern/NER/LLM downstream
//! see what it wrote.
//!
//! [`Enricher<Text>`]: elide_core::recognition::Enricher

use elide::detection::Analyzer;
use elide::enrichment::lingua::LinguaEnricher;
use elide_core::modality::text::Text;
use nvisy_schema::plan::LanguageEnricherParams;

/// Attach the lingua language-detection enricher built from `spec`.
///
/// An empty `candidates` list yields the unrestricted detector
/// (every language lingua was compiled with); a non-empty list
/// scopes detection to that pool.
pub(in crate::analyzer) fn attach(
    analyzer: Analyzer<Text>,
    spec: &LanguageEnricherParams,
) -> Analyzer<Text> {
    let enricher = if spec.candidates.is_empty() {
        LinguaEnricher::unrestricted()
    } else {
        LinguaEnricher::with_candidates(spec.candidates.iter().cloned())
    };
    analyzer.with_enricher(enricher)
}
