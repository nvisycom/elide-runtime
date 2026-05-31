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
use super::{AudialWorkflow, ExtractDispatch, Extraction, ExtractionEngine, WorkflowSlice};
use crate::envelope::DocumentEnvelope;

#[cfg(feature = "audio")]
#[async_trait::async_trait]
impl ExtractDispatch<Audio> for ExtractionEngine {
    type Workflow = AudialWorkflow;

    async fn extract(
        &self,
        envelope: &mut DocumentEnvelope<Audio>,
        workflow: &AudialWorkflow,
    ) -> Result<()> {
        if let Some(ref stt) = self.stt {
            stt.run(envelope, workflow.diarization).await?;
        }
        Ok(())
    }
}

#[cfg(not(feature = "audio"))]
#[async_trait::async_trait]
impl ExtractDispatch<Audio> for ExtractionEngine {
    type Workflow = AudialWorkflow;

    async fn extract(
        &self,
        _envelope: &mut DocumentEnvelope<Audio>,
        _workflow: &AudialWorkflow,
    ) -> Result<()> {
        Ok(())
    }
}

impl WorkflowSlice<Audio> for Extraction {
    fn slice(&self) -> &AudialWorkflow {
        &self.audial
    }
}
