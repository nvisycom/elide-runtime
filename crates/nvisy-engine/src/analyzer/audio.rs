//! Compile the audio-applicable parts of an
//! [`AnalyzerParams`] into an [`elide::detection::Analyzer<Audio>`].
//!
//! Audio runs Pattern and NER over the transcript text. The STT
//! enricher stamps `TranscriptSegment`s onto the recognizer
//! artifacts before recognition; it attaches when the deployment
//! wired one via [`Engine::with_stt`]. LLM has no `LlmModality`
//! impl for Audio in elide today.
//!
//! [`AnalyzerParams`]: nvisy_schema::plan::AnalyzerParams
//! [`Engine::with_stt`]: crate::Engine::with_stt

use elide::detection::Analyzer;
use elide_core::Result;
use elide_core::modality::audio::Audio;
use nvisy_schema::plan::AnalyzerParams;

use super::enricher::attach_stt;
use super::layer::attach_dedup;
use super::recognizer::{attach_ner_lineup, attach_pattern};
use crate::provider::ner::NerConfig;
use crate::provider::stt::SttBackend;

/// Compile `spec` into an audio-modality [`Analyzer`].
pub(super) fn compile(
    spec: &AnalyzerParams,
    ner: &NerConfig,
    stt: Option<&SttBackend>,
) -> Result<Analyzer<Audio>> {
    let mut analyzer = Analyzer::<Audio>::new();

    if let Some(stt) = stt {
        analyzer = attach_stt(analyzer, stt)?;
    }

    analyzer = attach_pattern(analyzer, &spec.recognizers)?;
    analyzer = attach_ner_lineup(analyzer, ner)?;

    Ok(attach_dedup(analyzer))
}
