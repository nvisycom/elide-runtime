//! Compile the text-applicable parts of [`AnalyzerParams`] into an
//! [`elide::detection::Analyzer<Text>`].
//!
//! Text supports the full recognizer set: Pattern, NER, and LLM.
//! NER and LLM are three-state toggles on
//! `spec.recognizers.{ner,llm}` (see
//! [`nvisy_schema::plan::RecognizerParams`]); the deployment's
//! [`NerConfig`] and [`LlmConfig`] provide the actual recognizer
//! lineups.
//!
//! Modality-foreign enrichers (`ocr`, `stt`) on `spec` are
//! silently ignored; those flow through the modalities they
//! belong to when an orchestrator pipeline encounters a body or
//! embedded part of that modality.
//!
//! [`AnalyzerParams`]: nvisy_schema::plan::AnalyzerParams
//! [`NerConfig`]: crate::provider::ner::NerConfig
//! [`LlmConfig`]: crate::provider::llm::LlmConfig

use elide::detection::Analyzer;
use elide_core::Error;
use elide_core::modality::text::Text;
use nvisy_schema::plan::AnalyzerParams;

use super::PatternGuardrails;
use super::enricher::attach_language;
use super::layer::attach_dedup;
use super::recognizer::{attach_llm_lineup, attach_ner_lineup, attach_pattern};
use crate::provider::llm::{AttachTo, LlmConfig};
use crate::provider::ner::NerConfig;

/// Compile `spec` into a text-modality [`Analyzer`]. Scope is
/// built separately and lives on the orchestrator.
pub(super) fn compile(
    spec: &AnalyzerParams,
    ner: &NerConfig,
    llm: &LlmConfig,
    guardrails: &PatternGuardrails,
) -> Result<Analyzer<Text>, Error> {
    let mut analyzer = Analyzer::<Text>::new();

    if let Some(language) = &spec.enrichers.language {
        analyzer = attach_language(analyzer, language);
    }

    if let Some(pattern) = &spec.recognizers.pattern {
        analyzer = attach_pattern(analyzer, pattern, guardrails)?;
    }
    analyzer = attach_ner_lineup(analyzer, ner, spec.recognizers.ner)?;
    analyzer = attach_llm_lineup(analyzer, llm, AttachTo::Text, spec.recognizers.llm)?;

    Ok(attach_dedup(analyzer, &spec.deduplication))
}
