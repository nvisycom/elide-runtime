//! Audio-modality extraction.
//!
//! Today's only audio extraction technique is STT ([`stt`]). Future
//! techniques (e.g. speaker diarization as its own pass) would live
//! as sibling sub-modules and stack inside this `ExtractDispatch<Audio>`
//! impl.

#[cfg(feature = "audio")]
pub mod stt;

use nvisy_core::Result;
use nvisy_ontology::modality::Audio;

#[cfg(feature = "audio")]
pub use self::stt::{SttExtractor, SttExtractorConfig};
use super::{AudioPlan, ExtractDispatch, Extraction, ExtractionEngine, PlanSlice};
use crate::envelope::DocumentEnvelope;

#[cfg(feature = "audio")]
#[async_trait::async_trait]
impl ExtractDispatch<Audio> for ExtractionEngine {
    type Plan = AudioPlan;

    async fn extract(
        &self,
        envelope: &mut DocumentEnvelope<Audio>,
        plan: &AudioPlan,
    ) -> Result<()> {
        if let Some(ref stt) = self.stt {
            stt.run(envelope, plan.diarization).await?;
        }
        Ok(())
    }
}

#[cfg(not(feature = "audio"))]
#[async_trait::async_trait]
impl ExtractDispatch<Audio> for ExtractionEngine {
    type Plan = AudioPlan;

    async fn extract(
        &self,
        _envelope: &mut DocumentEnvelope<Audio>,
        _plan: &AudioPlan,
    ) -> Result<()> {
        Ok(())
    }
}

impl PlanSlice<Audio> for Extraction {
    fn slice(&self) -> &AudioPlan {
        &self.audio
    }
}
