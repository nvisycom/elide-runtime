//! [`ExtractorRegistry`]: per-modality container of pre-built
//! extractors keyed by modality.
//!
//! At most one extractor per modality today (OCR for [`Image`], STT
//! for [`Audio`]). The output shape is pinned per modality so the
//! document-side glue knows how to map the extractor's `Output` into
//! `Block<M>` values — OCR backends produce `Vec<OcrOutput>`, STT
//! backends produce `SttOutput`. Custom backends slot in by
//! implementing the matching [`Extractor`] trait shape.
//!
//! Toolkit holds no `from_config` builder — config-driven construction
//! is the consumer's concern. The pipeline layer (`nvisy-document`)
//! deserialises the `[extractor.*]` sections and inserts the chosen
//! [`Arc<dyn Extractor<M>>`] into the registry.
//!
//! [`Extractor`]: nvisy_core::Extractor

use std::sync::Arc;

use nvisy_core::Extractor;
use nvisy_core::modality::{Audio, Image};

/// Output shape produced by every image-modality extractor.
///
/// Pinned to [`Vec<OcrOutput>`] today; widening the type later (to
/// accommodate scene-text / layout-detection backends with different
/// shapes) is a breaking change worth taking deliberately rather than
/// hiding behind generics.
#[cfg(feature = "image")]
pub type ImageExtractorOutput = Vec<nvisy_ocr::core::OcrOutput>;

/// Output shape produced by every audio-modality extractor.
#[cfg(feature = "audio")]
pub type AudioExtractorOutput = nvisy_agent::audio::stt::SttOutput;

/// Per-modality container of pre-built extractors.
///
/// Built once at engine startup, cloned per run (the inner `Arc`s
/// keep the actual extractors shared without copying).
#[derive(Default, Clone)]
pub struct ExtractorRegistry {
    /// Image-modality extractor slot. `None` opts the technique out
    /// entirely.
    #[cfg(feature = "image")]
    pub image: Option<Arc<dyn Extractor<Image, Output = ImageExtractorOutput>>>,
    /// Audio-modality extractor slot. `None` opts the technique out
    /// entirely.
    #[cfg(feature = "audio")]
    pub audio: Option<Arc<dyn Extractor<Audio, Output = AudioExtractorOutput>>>,
}

impl ExtractorRegistry {
    /// Build an empty registry. Useful for tests; production callers
    /// populate the slots with [`with_image_extractor`] /
    /// [`with_audio_extractor`] after constructing the chosen
    /// backend.
    ///
    /// [`with_image_extractor`]: Self::with_image_extractor
    /// [`with_audio_extractor`]: Self::with_audio_extractor
    pub fn new() -> Self {
        Self::default()
    }

    /// Install the image-modality extractor. Takes ownership of the
    /// extractor and wraps it in `Arc` internally.
    #[cfg(feature = "image")]
    #[must_use]
    pub fn with_image_extractor<E>(mut self, extractor: E) -> Self
    where
        E: Extractor<Image, Output = ImageExtractorOutput> + 'static,
    {
        self.image = Some(Arc::new(extractor));
        self
    }

    /// Install the audio-modality extractor. Takes ownership of the
    /// extractor and wraps it in `Arc` internally.
    #[cfg(feature = "audio")]
    #[must_use]
    pub fn with_audio_extractor<E>(mut self, extractor: E) -> Self
    where
        E: Extractor<Audio, Output = AudioExtractorOutput> + 'static,
    {
        self.audio = Some(Arc::new(extractor));
        self
    }

    /// `true` when no extractor is installed.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        #[cfg(feature = "image")]
        let image_empty = self.image.is_none();
        #[cfg(not(feature = "image"))]
        let image_empty = true;
        #[cfg(feature = "audio")]
        let audio_empty = self.audio.is_none();
        #[cfg(not(feature = "audio"))]
        let audio_empty = true;
        image_empty && audio_empty
    }
}
