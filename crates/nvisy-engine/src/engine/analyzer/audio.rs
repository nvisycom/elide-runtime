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
//! [`AnalyzerParams`]: nvisy_core::plan::AnalyzerParams

use elide::detection::Analyzer;
use elide_core::modality::audio::Audio;
use elide_core::{Error, ErrorKind};
use elide_stt::{MockBackend as MockSttBackend, SttEnricher};
use nvisy_core::plan::{AnalyzerParams, SttBackendParams, SttEnricherParams};

use super::common::{attach_dedup, attach_ner, attach_pattern};

/// Compile `spec` into an audio-modality [`Analyzer`].
pub(super) fn compile(spec: &AnalyzerParams) -> Result<Analyzer<Audio>, Error> {
    let mut analyzer = Analyzer::<Audio>::new();

    if let Some(stt) = &spec.enrichers.stt {
        analyzer = attach_stt(analyzer, stt)?;
    }

    if let Some(pattern) = &spec.recognizers.pattern {
        analyzer = attach_pattern(analyzer, pattern)?;
    }
    for ner in &spec.recognizers.ner {
        analyzer = attach_ner(analyzer, ner)?;
    }

    Ok(attach_dedup(analyzer, &spec.deduplication))
}

/// Attach an [`SttEnricher`] for the audio modality. `Mock` uses
/// elide-stt's in-process no-op backend; `Bento` returns a clean
/// "not wired yet" error until `elide-bento` ships a `BentoStt`
/// client.
fn attach_stt(
    analyzer: Analyzer<Audio>,
    spec: &SttEnricherParams,
) -> Result<Analyzer<Audio>, Error> {
    match &spec.backend {
        SttBackendParams::Mock => Ok(analyzer.with_enricher(SttEnricher::new(MockSttBackend))),
        SttBackendParams::Bento { .. } => Err(Error::new(
            ErrorKind::Validation,
            "analyzer compile: BentoML STT backend needs an elide-bento `BentoStt` \
             client; not wired into the compile surface yet",
        )),
    }
}
