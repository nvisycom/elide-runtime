//! Compile the tabular-applicable parts of an
//! [`AnalyzerParams`] into an
//! [`elide::detection::Analyzer<Tabular>`].
//!
//! Tabular runs Pattern and NER over each cell's text (cells
//! are `TextRecognizable`). LLM has no `LlmModality` impl for
//! Tabular in elide today.
//!
//! [`AnalyzerParams`]: nvisy_schema::plan::AnalyzerParams

use elide::detection::Analyzer;
use elide_core::Result;
use elide_core::modality::tabular::Tabular;
use nvisy_schema::plan::AnalyzerParams;

use super::layer::attach_dedup;
use super::recognizer::{attach_ner_lineup, attach_pattern};
use crate::provider::ner::NerConfig;

/// Compile `spec` into a tabular-modality [`Analyzer`].
pub(super) fn compile(spec: &AnalyzerParams, ner: &NerConfig) -> Result<Analyzer<Tabular>> {
    let mut analyzer = Analyzer::<Tabular>::new();

    analyzer = attach_pattern(analyzer, &spec.recognizers)?;
    analyzer = attach_ner_lineup(analyzer, ner)?;

    Ok(attach_dedup(analyzer))
}
