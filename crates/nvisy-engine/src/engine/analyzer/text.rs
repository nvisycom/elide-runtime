//! Compile the text-applicable parts of [`AnalyzerParams`] into an
//! [`elide::detection::Analyzer<Text>`].
//!
//! Text supports the full recognizer set: Pattern, NER, and LLM.
//! LLM is opt-in via `spec.recognizers.llm = true`; the
//! deployment's [`LlmConfig`] provides the actual recognizer
//! lineup.
//!
//! Modality-foreign enrichers (`ocr`, `stt`) on `spec` are
//! silently ignored — those flow through the modalities they
//! belong to when an orchestrator pipeline encounters a body or
//! embedded part of that modality.
//!
//! [`AnalyzerParams`]: nvisy_core::plan::AnalyzerParams
//! [`LlmConfig`]: nvisy_core::llm::LlmConfig

use elide::detection::Analyzer;
use elide_core::Error;
use elide_core::modality::text::Text;
use nvisy_core::llm::{LlmConfig, LlmRecognizerModality};
use nvisy_core::plan::AnalyzerParams;

use super::common::{attach_dedup, attach_language, attach_ner, attach_pattern};
use crate::llm::attach_lineup;

/// Compile `spec` into a text-modality [`Analyzer`]. Scope is
/// built separately and lives on the orchestrator.
pub(super) fn compile(spec: &AnalyzerParams, llm: &LlmConfig) -> Result<Analyzer<Text>, Error> {
    let mut analyzer = Analyzer::<Text>::new();

    if let Some(language) = &spec.enrichers.language {
        analyzer = attach_language(analyzer, language);
    }

    if let Some(pattern) = &spec.recognizers.pattern {
        analyzer = attach_pattern(analyzer, pattern)?;
    }
    for ner in &spec.recognizers.ner {
        analyzer = attach_ner(analyzer, ner)?;
    }
    if spec.recognizers.llm {
        analyzer = attach_lineup(analyzer, llm, LlmRecognizerModality::Text)?;
    }

    Ok(attach_dedup(analyzer, &spec.deduplication))
}
