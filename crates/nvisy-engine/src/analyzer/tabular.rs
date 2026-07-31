//! Compile the tabular-applicable parts of [`AnalyzerParams`]
//! into an [`elide::detection::Analyzer<Tabular>`].
//!
//! Tabular runs Pattern and NER over each cell's text (cells
//! are `TextRecognizable`). LLM has no `LlmModality` impl for
//! Tabular in elide today, so `recognizers.llm` is silently
//! ignored here. (ELIDE GAP: an `LlmModality` impl would let an
//! LLM scan tables for PII.)
//!
//! Modality-foreign enrichers (`language`, `ocr`, `stt`) are
//! silently ignored too — those flow through the modalities they
//! belong to when an orchestrator pipeline encounters a body or
//! embedded part of that modality.
//!
//! [`AnalyzerParams`]: nvisy_schema::plan::AnalyzerParams

use elide::detection::Analyzer;
use elide_core::Error;
use elide_core::modality::tabular::Tabular;
use nvisy_schema::plan::AnalyzerParams;

use super::PatternGuardrails;
use super::layer::attach_dedup;
use super::recognizer::{attach_ner_lineup, attach_pattern};
use crate::provider::ner::NerConfig;

/// Compile `spec` into a tabular-modality [`Analyzer`].
pub(super) fn compile(
    spec: &AnalyzerParams,
    ner: &NerConfig,
    guardrails: &PatternGuardrails,
) -> Result<Analyzer<Tabular>, Error> {
    let mut analyzer = Analyzer::<Tabular>::new();

    if let Some(pattern) = &spec.recognizers.pattern {
        analyzer = attach_pattern(analyzer, pattern, guardrails)?;
    }
    analyzer = attach_ner_lineup(analyzer, ner, spec.recognizers.ner.as_ref())?;

    Ok(attach_dedup(analyzer, &spec.deduplication))
}
