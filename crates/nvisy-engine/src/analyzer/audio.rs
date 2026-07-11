//! Compile the audio-applicable parts of [`AnalyzerParams`]
//! into an [`elide::detection::Analyzer<Audio>`].
//!
//! Audio runs Pattern and NER over the transcript text. An STT
//! enricher stamps `TranscriptSegment`s onto the recognizer
//! artifacts before recognition; the per-modality compile path
//! wires it via `elide-stt`. LLM has no `LlmModality` impl for
//! Audio in elide today, so `recognizers.llm` is silently
//! ignored here.
//!
//! Modality-foreign enrichers (`language`, `ocr`) on `spec` are
//! silently ignored too — those flow through the modalities they
//! belong to when an orchestrator pipeline encounters a body or
//! embedded part of that modality.
//!
//! [`AnalyzerParams`]: nvisy_schema::plan::AnalyzerParams

use crate::provider::ner::NerConfig;
use elide::detection::Analyzer;
use elide_core::Error;
use elide_core::modality::audio::Audio;
use nvisy_schema::plan::AnalyzerParams;

use super::PatternGuardrails;
use super::enricher::attach_stt;
use super::layer::attach_dedup;
use super::recognizer::{attach_ner_lineup, attach_pattern};

/// Compile `spec` into an audio-modality [`Analyzer`].
pub(super) fn compile(
    spec: &AnalyzerParams,
    ner: &NerConfig,
    guardrails: &PatternGuardrails,
) -> Result<Analyzer<Audio>, Error> {
    let mut analyzer = Analyzer::<Audio>::new();

    if let Some(stt) = &spec.enrichers.stt {
        analyzer = attach_stt(analyzer, stt)?;
    }

    if let Some(pattern) = &spec.recognizers.pattern {
        analyzer = attach_pattern(analyzer, pattern, guardrails)?;
    }
    analyzer = attach_ner_lineup(analyzer, ner, spec.recognizers.ner)?;

    Ok(attach_dedup(analyzer, &spec.deduplication))
}
