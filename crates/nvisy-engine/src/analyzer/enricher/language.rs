//! Attach the lingua language-detection [`Enricher<Text>`].
//!
//! Text-modality only — writes the detected language into the
//! per-request recognizer context, so pattern/NER/LLM downstream
//! see what it wrote. The detector considers every language
//! lingua was compiled with.
//!
//! [`Enricher<Text>`]: elide_core::recognition::Enricher

use elide::detection::Analyzer;
use elide::enrichment::lingua::LinguaEnricher;
use elide_core::modality::text::Text;

/// Attach the lingua language-detection enricher.
pub(in crate::analyzer) fn attach(analyzer: Analyzer<Text>) -> Analyzer<Text> {
    analyzer.with_enricher(LinguaEnricher::unrestricted())
}
