//! Compile the audio-applicable parts of [`AnalyzerParams`]
//! into an [`elide::detection::Analyzer<Audio>`].
//!
//! Audio runs Pattern and NER over the transcript text. An STT
//! enricher stamps `TranscriptSegment`s onto the recognizer
//! artifacts before recognition; the per-modality compile path
//! wires it via `elide-stt`. LLM is not implemented on Audio in
//! elide today — `RecognizerParams::llm` returns a `Validation`
//! error.
//!
//! [`AnalyzerParams`]: nvisy_core::plan::AnalyzerParams

use elide::detection::Analyzer;
use elide_core::modality::audio::Audio;
use elide_core::{Error, ErrorKind};
use elide_stt::{MockBackend as MockSttBackend, SttEnricher};
use nvisy_core::plan::{AnalyzerParams, SttBackendParams, SttEnricherParams};

use super::common::{attach_dedup, attach_ner, attach_pattern, reject_language_enricher};

/// Compile `spec` into an audio-modality [`Analyzer`].
pub(crate) fn compile_audio(spec: &AnalyzerParams) -> Result<Analyzer<Audio>, Error> {
    let mut analyzer = Analyzer::<Audio>::new();

    if spec.enrichers.language.is_some() {
        analyzer = reject_language_enricher::<Audio>()?;
    }
    if spec.enrichers.ocr.is_some() {
        return Err(Error::new(
            ErrorKind::Validation,
            "analyzer compile: OCR enricher is only valid on the image modality",
        ));
    }
    if let Some(stt) = &spec.enrichers.stt {
        analyzer = attach_stt(analyzer, stt)?;
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
            "analyzer compile: LLM recognizer is not available on the audio modality \
             (elide-llm has no LlmModality impl for Audio today)",
        ));
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
