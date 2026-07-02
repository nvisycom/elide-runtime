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

use elide::detection::Analyzer;
use elide_core::modality::audio::Audio;
use elide_core::{Error, ErrorKind};
#[cfg(feature = "test-utils")]
use elide_stt::{MockBackend as MockSttBackend, SttEnricher};
use nvisy_core::ner::NerConfig;
use nvisy_schema::plan::{AnalyzerParams, SttBackendParams, SttEnricherParams};

use super::common::{attach_dedup, attach_ner_lineup, attach_pattern};

/// Compile `spec` into an audio-modality [`Analyzer`].
pub(super) fn compile(spec: &AnalyzerParams, ner: &NerConfig) -> Result<Analyzer<Audio>, Error> {
    let mut analyzer = Analyzer::<Audio>::new();

    if let Some(stt) = &spec.enrichers.stt {
        analyzer = attach_stt(analyzer, stt)?;
    }

    if let Some(pattern) = &spec.recognizers.pattern {
        analyzer = attach_pattern(analyzer, pattern)?;
    }
    if spec.recognizers.ner {
        analyzer = attach_ner_lineup(analyzer, ner)?;
    }

    Ok(attach_dedup(analyzer, &spec.deduplication))
}

/// Attach an [`SttEnricher`] for the audio modality. The
/// deployment's `Bento` backend returns a clean "not wired yet"
/// error until `elide-bento` ships a `BentoStt` client;
/// unknown variants surface as Validation.
///
/// [`SttEnricher`]: elide_stt::SttEnricher
fn attach_stt(
    analyzer: Analyzer<Audio>,
    spec: &SttEnricherParams,
) -> Result<Analyzer<Audio>, Error> {
    #[cfg(not(feature = "test-utils"))]
    let _ = analyzer;
    match &spec.backend {
        SttBackendParams::Bento { .. } => Err(Error::new(
            ErrorKind::Validation,
            "analyzer compile: BentoML STT backend needs an elide-bento `BentoStt` \
             client; not wired into the compile surface yet",
        )),
        #[cfg(feature = "test-utils")]
        SttBackendParams::Mock => Ok(analyzer.with_enricher(SttEnricher::new(MockSttBackend))),
        // `SttBackendParams` is `#[non_exhaustive]`. Unknown
        // variants surface as Validation.
        _ => Err(Error::new(
            ErrorKind::Validation,
            "analyzer compile: STT enricher uses a backend kind this engine binary \
             doesn't understand; upgrade the engine or downgrade the config",
        )),
    }
}
