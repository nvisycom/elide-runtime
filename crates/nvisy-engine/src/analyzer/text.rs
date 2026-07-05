//! Compile the text-applicable parts of [`AnalyzerParams`] into an
//! [`elide::detection::Analyzer<Text>`].
//!
//! Text supports the full recognizer set: Pattern, NER, and LLM.
//! NER and LLM are opt-in via `spec.recognizers.ner = true` /
//! `spec.recognizers.llm = true`; the deployment's [`NerConfig`]
//! and [`LlmConfig`] provide the actual recognizer lineups.
//!
//! Modality-foreign enrichers (`ocr`, `stt`) on `spec` are
//! silently ignored; those flow through the modalities they
//! belong to when an orchestrator pipeline encounters a body or
//! embedded part of that modality.
//!
//! [`AnalyzerParams`]: nvisy_schema::plan::AnalyzerParams
//! [`NerConfig`]: nvisy_core::ner::NerConfig
//! [`LlmConfig`]: nvisy_core::llm::LlmConfig

use elide::detection::Analyzer;
use elide_core::Error;
use elide_core::modality::text::Text;
use nvisy_core::llm::{LlmConfig, LlmRecognizerModality};
use nvisy_core::ner::NerConfig;
use nvisy_schema::plan::AnalyzerParams;

use super::common::{attach_dedup, attach_language, attach_pattern};
use super::llm::attach_llm_lineup;
use super::ner::attach_ner_lineup;

/// Compile `spec` into a text-modality [`Analyzer`]. Scope is
/// built separately and lives on the orchestrator.
pub(super) fn compile(
    spec: &AnalyzerParams,
    ner: &NerConfig,
    llm: &LlmConfig,
) -> Result<Analyzer<Text>, Error> {
    let mut analyzer = Analyzer::<Text>::new();

    if let Some(language) = &spec.enrichers.language {
        analyzer = attach_language(analyzer, language);
    }

    if let Some(pattern) = &spec.recognizers.pattern {
        analyzer = attach_pattern(analyzer, pattern)?;
    }
    if spec.recognizers.ner {
        analyzer = attach_ner_lineup(analyzer, ner)?;
    }
    if spec.recognizers.llm {
        analyzer = attach_llm_lineup(analyzer, llm, LlmRecognizerModality::Text)?;
    }

    Ok(attach_dedup(analyzer, &spec.deduplication))
}
