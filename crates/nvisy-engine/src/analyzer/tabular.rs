//! Compile the tabular-applicable parts of [`AnalyzerParams`]
//! into an [`elide::detection::Analyzer<Tabular>`].
//!
//! Tabular runs Pattern and NER over each cell's text (cells
//! are `TextRecognizable`). LLM is not implemented on Tabular in
//! elide today — `RecognizerParams::llm` returns a `Validation`
//! error. (ELIDE GAP: `impl LlmModality for Tabular` would let
//! an LLM scan tables for PII.)
//!
//! [`AnalyzerParams`]: nvisy_core::plan::AnalyzerParams

use elide::detection::Analyzer;
use elide_core::modality::tabular::Tabular;
use elide_core::recognition::Scope;
use elide_core::{Error, ErrorKind};
use nvisy_core::plan::AnalyzerParams;

use super::common::{
    attach_dedup, attach_ner, attach_pattern, build_catalog, reject_language_enricher,
};
use super::scope::compile_scope;

/// Compile `spec` into a tabular-modality analyzer + its
/// compiled [`Scope`].
pub fn compile_tabular(
    spec: &AnalyzerParams,
) -> Result<(Analyzer<Tabular>, Scope<Tabular>), Error> {
    let scope = compile_scope::<Tabular>(&spec.scope)?;
    let catalog = build_catalog(spec);
    let mut analyzer = Analyzer::<Tabular>::new();

    if spec.enrichers.language.is_some() {
        analyzer = reject_language_enricher::<Tabular>()?;
    }
    if spec.enrichers.ocr.is_some() {
        return Err(Error::new(
            ErrorKind::Validation,
            "analyzer compile: OCR enricher is only valid on the image modality",
        ));
    }
    if spec.enrichers.stt.is_some() {
        return Err(Error::new(
            ErrorKind::Validation,
            "analyzer compile: STT enricher is only valid on the audio modality",
        ));
    }

    if let Some(pattern) = &spec.recognizers.pattern {
        analyzer = attach_pattern(analyzer, pattern)?;
    }
    for ner in &spec.recognizers.ner {
        analyzer = attach_ner(analyzer, ner)?;
    }
    if !spec.recognizers.llm.is_empty() {
        return Err(Error::new(
            ErrorKind::Validation,
            "analyzer compile: LLM recognizer is not available on the tabular \
             modality (elide-llm has no LlmModality impl for Tabular today)",
        ));
    }

    analyzer = attach_dedup(analyzer, &spec.deduplication);
    let _ = catalog;
    Ok((analyzer, scope))
}
