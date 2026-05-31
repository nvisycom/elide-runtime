//! Image-modality extraction.
//!
//! Today's only image extraction technique is OCR ([`ocr`]). Future
//! techniques (e.g. layout segmentation, scene-text detection) would
//! live as sibling sub-modules and stack inside this `ExtractDispatch<Image>`
//! impl.

#[cfg(feature = "image")]
pub mod ocr;

use nvisy_core::Result;
use nvisy_ontology::modality::Image;

#[cfg(feature = "image")]
pub use self::ocr::{OcrExtractor, OcrExtractorConfig};
use super::{ExtractDispatch, Extraction, ExtractionEngine, ImagePlan, PlanSlice};
use crate::envelope::DocumentEnvelope;

#[cfg(feature = "image")]
#[async_trait::async_trait]
impl ExtractDispatch<Image> for ExtractionEngine {
    type Plan = ImagePlan;

    async fn extract(
        &self,
        envelope: &mut DocumentEnvelope<Image>,
        _plan: &ImagePlan,
    ) -> Result<()> {
        if let Some(ref ocr) = self.ocr {
            ocr.run(envelope).await?;
        }
        Ok(())
    }
}

#[cfg(not(feature = "image"))]
#[async_trait::async_trait]
impl ExtractDispatch<Image> for ExtractionEngine {
    type Plan = ImagePlan;

    async fn extract(
        &self,
        _envelope: &mut DocumentEnvelope<Image>,
        _plan: &ImagePlan,
    ) -> Result<()> {
        Ok(())
    }
}

impl PlanSlice<Image> for Extraction {
    fn slice(&self) -> &ImagePlan {
        &self.image
    }
}
