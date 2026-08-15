//! Per-modality [`Analyzer`] compile functions.
//!
//! One function per modality: each picks the recognizers and
//! enrichers the modality supports and rejects the rest at
//! compile time (e.g. OCR on text, LLM on tabular). Every
//! compile fn consults the deployment [`NerConfig`]; [`compile_text`]
//! and [`compile_image`] also consult [`LlmConfig`]. The
//! language-detection enricher always attaches to text; OCR and
//! STT enrichers attach when their per-modality backend is wired
//! on the engine.
//!
//! Non-text methods are gated on their modality's feature.
//!
//! [`Analyzer`]: elide::detection::Analyzer
//! [`LlmConfig`]: crate::provider::llm::LlmConfig
//! [`NerConfig`]: crate::provider::ner::NerConfig

use elide::detection::Analyzer;
use elide_core::Result;
#[cfg(feature = "internal_audio")]
use elide_core::modality::audio::Audio;
#[cfg(feature = "internal_image")]
use elide_core::modality::image::Image;
#[cfg(feature = "internal_tabular")]
use elide_core::modality::tabular::Tabular;
use elide_core::modality::text::Text;

use super::enrichers::attach_language;
#[cfg(feature = "internal_image")]
use super::enrichers::attach_ocr;
#[cfg(feature = "internal_audio")]
use super::enrichers::attach_stt;
use super::layer::attach_dedup;
use super::recognizers::{attach_llm_lineup, attach_ner_lineup, attach_pattern};
use crate::provider::llm::{AttachTo, LlmConfig};
use crate::provider::ner::NerConfig;
#[cfg(feature = "internal_image")]
use crate::provider::ocr::OcrBackend;
#[cfg(feature = "internal_audio")]
use crate::provider::stt::SttBackend;

/// Compile a text-modality [`Analyzer`].
///
/// Text supports the full recognizer set: Pattern, NER, and LLM.
/// Every wired NER and LLM recognizer whose modality list
/// contains `Text` attaches. Language detection always attaches.
pub(crate) fn compile_text(ner: &NerConfig, llm: &LlmConfig) -> Result<Analyzer<Text>> {
    let mut analyzer = Analyzer::<Text>::new();

    analyzer = attach_language(analyzer);
    analyzer = attach_pattern(analyzer)?;
    analyzer = attach_ner_lineup(analyzer, ner)?;
    analyzer = attach_llm_lineup(analyzer, llm, AttachTo::Text)?;

    Ok(attach_dedup(analyzer))
}

/// Compile a tabular-modality [`Analyzer`].
///
/// Tabular runs Pattern and NER over each cell's text (cells
/// are `TextRecognizable`). LLM has no `LlmModality` impl for
/// Tabular in elide today.
#[cfg(feature = "internal_tabular")]
pub(crate) fn compile_tabular(ner: &NerConfig) -> Result<Analyzer<Tabular>> {
    let mut analyzer = Analyzer::<Tabular>::new();

    analyzer = attach_pattern(analyzer)?;
    analyzer = attach_ner_lineup(analyzer, ner)?;

    Ok(attach_dedup(analyzer))
}

/// Compile an image-modality [`Analyzer`].
///
/// Image is the fullest non-text modality: Pattern and NER run
/// over the OCR'd text (the OCR enricher stamps a `Layout` onto
/// the recognizer artifacts upstream), and LLM is available
/// image-natively for vision-language models. The OCR enricher
/// attaches when the deployment wired one via
/// [`Engine::with_ocr`].
///
/// [`Engine::with_ocr`]: crate::Engine::with_ocr
#[cfg(feature = "internal_image")]
pub(crate) fn compile_image(
    ner: &NerConfig,
    llm: &LlmConfig,
    ocr: Option<&OcrBackend>,
) -> Result<Analyzer<Image>> {
    let mut analyzer = Analyzer::<Image>::new();

    if let Some(ocr) = ocr {
        analyzer = attach_ocr(analyzer, ocr)?;
    }

    analyzer = attach_pattern(analyzer)?;
    analyzer = attach_ner_lineup(analyzer, ner)?;
    analyzer = attach_llm_lineup(analyzer, llm, AttachTo::Image)?;

    Ok(attach_dedup(analyzer))
}

/// Compile an audio-modality [`Analyzer`].
///
/// Audio runs Pattern and NER over the transcript text. The STT
/// enricher stamps `TranscriptSegment`s onto the recognizer
/// artifacts before recognition; it attaches when the deployment
/// wired one via [`Engine::with_stt`]. LLM has no `LlmModality`
/// impl for Audio in elide today.
///
/// [`Engine::with_stt`]: crate::Engine::with_stt
#[cfg(feature = "internal_audio")]
pub(crate) fn compile_audio(ner: &NerConfig, stt: Option<&SttBackend>) -> Result<Analyzer<Audio>> {
    let mut analyzer = Analyzer::<Audio>::new();

    if let Some(stt) = stt {
        analyzer = attach_stt(analyzer, stt)?;
    }

    analyzer = attach_pattern(analyzer)?;
    analyzer = attach_ner_lineup(analyzer, ner)?;

    Ok(attach_dedup(analyzer))
}
