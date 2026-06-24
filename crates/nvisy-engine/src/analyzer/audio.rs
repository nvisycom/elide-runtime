//! Compile the audio-applicable parts of [`AnalyzerSpec`] into an
//! [`elide::Analyzer<Audio>`].
//!
//! Audio runs Pattern and NER over the transcript text (an enricher
//! must stamp a `Transcription` onto the recognizer artifacts
//! upstream). LLM is not implemented on Audio in elide today —
//! `RecognizerSpec::Llm` returns a `Validation` error. (ELIDE GAP:
//! `impl LlmModality for Audio` would let an audio-native LLM
//! recognize entities in audio.)
//!
//! [`AnalyzerSpec`]: nvisy_core::plan::AnalyzerSpec

use elide::Analyzer;
use elide_core::modality::audio::Audio;
use elide_core::recognition::Scope;
use elide_core::{Error, ErrorKind};
use nvisy_core::plan::{AnalyzerSpec, RecognizerSpec};

use super::common::{attach_dedup, attach_enricher, attach_ner, attach_pattern, build_catalog};
use super::scope::compile_scope;

/// Compile `spec` into an audio-modality analyzer + its compiled
/// [`Scope`].
pub fn compile_audio(
    spec: &AnalyzerSpec,
) -> Result<(Analyzer<Audio>, Scope<Audio>), Error> {
    let scope = compile_scope::<Audio>(&spec.scope)?;
    let catalog = build_catalog(spec);
    let mut analyzer = Analyzer::<Audio>::new();

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
                    "analyzer compile: LLM recognizer is not available on the audio \
                     modality (elide-llm has no LlmModality impl for Audio today)",
                ));
            }
        };
    }

    analyzer = attach_dedup(analyzer, &spec.deduplication);
    let _ = catalog;
    Ok((analyzer, scope))
}
