//! Compile the tabular-applicable parts of [`AnalyzerSpec`] into
//! an [`elide::Analyzer<Tabular>`].
//!
//! Tabular runs Pattern and NER over each cell's text (cells are
//! `TextRecognizable`). LLM is not implemented on Tabular in elide
//! today — `RecognizerSpec::Llm` returns a `Validation` error.
//! (ELIDE GAP: `impl LlmModality for Tabular` would let an LLM
//! scan tables for PII.)
//!
//! [`AnalyzerSpec`]: nvisy_core::plan::AnalyzerSpec

use elide::Analyzer;
use elide_core::modality::tabular::Tabular;
use elide_core::recognition::Scope;
use elide_core::{Error, ErrorKind};
use nvisy_core::plan::{AnalyzerSpec, RecognizerSpec};

use super::common::{attach_dedup, attach_enricher, attach_ner, attach_pattern, build_catalog};
use super::scope::compile_scope;

/// Compile `spec` into a tabular-modality analyzer + its compiled
/// [`Scope`].
pub fn compile_tabular(
    spec: &AnalyzerSpec,
) -> Result<(Analyzer<Tabular>, Scope<Tabular>), Error> {
    let scope = compile_scope::<Tabular>(&spec.scope)?;
    let catalog = build_catalog(spec);
    let mut analyzer = Analyzer::<Tabular>::new();

    for enricher in &spec.enrichers {
        analyzer = attach_enricher(analyzer, enricher)?;
    }

    for recognizer in &spec.recognizers {
        analyzer = match recognizer {
            RecognizerSpec::Pattern(p) => attach_pattern(analyzer, p)?,
            RecognizerSpec::Ner(n) => attach_ner(analyzer, n)?,
            RecognizerSpec::Llm(_) => {
                return Err(Error::new(
                    ErrorKind::Validation,
                    "analyzer compile: LLM recognizer is not available on the tabular \
                     modality (elide-llm has no LlmModality impl for Tabular today)",
                ));
            }
        };
    }

    analyzer = attach_dedup(analyzer, &spec.deduplication);
    let _ = catalog;
    Ok((analyzer, scope))
}
