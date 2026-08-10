//! Compile the text-applicable parts of an
//! [`AnalyzerParams`] into an [`elide::detection::Analyzer<Text>`].
//!
//! Text supports the full recognizer set: Pattern, NER, and LLM.
//! Every wired NER and LLM recognizer whose modality list
//! contains `Text` attaches; the deployment picks the lineup via
//! [`Engine::with_ner`] / [`Engine::with_llm`]. The
//! language-detection enricher always attaches.
//!
//! [`AnalyzerParams`]: nvisy_schema::plan::AnalyzerParams
//! [`Engine::with_llm`]: crate::Engine::with_llm
//! [`Engine::with_ner`]: crate::Engine::with_ner

use elide::detection::Analyzer;
use elide_core::Result;
use elide_core::modality::text::Text;
use nvisy_schema::plan::AnalyzerParams;

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
) -> Result<Analyzer<Text>> {
    let mut analyzer = Analyzer::<Text>::new();

    analyzer = attach_language(analyzer);
    analyzer = attach_pattern(analyzer, &spec.recognizers)?;
    analyzer = attach_ner_lineup(analyzer, ner)?;
    analyzer = attach_llm_lineup(analyzer, llm, AttachTo::Text)?;

    Ok(attach_dedup(analyzer))
}
