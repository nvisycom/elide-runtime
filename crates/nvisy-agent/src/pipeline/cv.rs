//! [`CvPipeline`]: image-side LLM work — classification + verification.
//!
//! Bundles a [`CvAgent`] (classifies pre-computed CV detections
//! into entity categories) with a [`CvVerifyAgent`] (image-side LLM
//! validation of entity proposals). Both flows are exposed
//! independently — the pipeline does not force run-them-together
//! semantics, because callers come in two shapes:
//!
//! - Fresh CV detection: caller has an image and raw detections
//!   from an upstream face/plate/signature detector. Call
//!   [`classify`] to turn detections into [`CvEntity`]s.
//! - Verifier-only: caller has entity proposals from anywhere
//!   (OCR + NER, prior CV runs) and an image. Call [`verify`] to
//!   validate them. The classification agent is optional in this
//!   case — construct with `agent_config = None` to skip building
//!   the agent half.
//!
//! Both methods are stateless across calls, so [`reset`] only
//! zeroes the cumulative usage trackers.
//!
//! [`CvAgent`]: crate::agent::cv::CvAgent
//! [`CvVerifyAgent`]: crate::agent::cv::CvVerifyAgent
//! [`reset`]: CvPipeline::reset
//! [`CvEntity`]: crate::agent::cv::CvEntity
//! [`classify`]: CvPipeline::classify
//! [`verify`]: CvPipeline::verify

use bytes::Bytes;
use nvisy_core::{Error, ErrorKind, Result};
use nvisy_ontology::entity::Entity;

use crate::agent::cv::{CvAgent, CvDetection, CvEntity, CvVerifyAgent, VerificationCandidate};
use crate::agent::{AgentConfig, AgentProvider, DetectionConfig, UsageStats};

/// Combined CV pipeline. Holds a [`CvVerifyAgent`] and an optional
/// [`CvAgent`] for classification. See module docs for the two
/// flow shapes.
pub struct CvPipeline {
    agent: Option<CvAgent>,
    verifier: CvVerifyAgent,
}

impl CvPipeline {
    /// Build a pipeline from an LLM provider plus agent configs.
    ///
    /// `agent_config` drives the classification agent — pass
    /// `None` to build a verifier-only pipeline (calling
    /// [`classify`] then errors). `verifier_config` drives the
    /// verifier agent. Both LLM calls can use independent models /
    /// temperatures via their configs.
    ///
    /// # Errors
    ///
    /// Returns an error if either underlying agent cannot be
    /// constructed.
    ///
    /// [`classify`]: Self::classify
    pub fn new(
        provider: &AgentProvider,
        agent_config: Option<AgentConfig>,
        verifier_config: AgentConfig,
    ) -> Result<Self> {
        let agent = match agent_config {
            Some(cfg) => Some(CvAgent::new(provider, cfg)?),
            None => None,
        };
        let verifier = CvVerifyAgent::new(provider, verifier_config)?;
        Ok(Self { agent, verifier })
    }

    /// Classify pre-computed CV detections into entity categories.
    ///
    /// `detections` come from an upstream CV backend (face / plate
    /// / signature detector). The pipeline's [`CvAgent`] adds VLM
    /// category labels. Empty input returns empty without prompting
    /// the LLM.
    ///
    /// Note: this method does not invoke the verifier. Run
    /// [`verify`] separately with the classified entities lifted
    /// into [`VerificationCandidate`]s if you want image-side
    /// validation.
    ///
    /// # Errors
    ///
    /// Returns an error if this pipeline was constructed without an
    /// `agent_config` (verifier-only mode).
    ///
    /// [`CvAgent`]: crate::agent::cv::CvAgent
    /// [`verify`]: Self::verify
    /// [`VerificationCandidate`]: crate::agent::cv::VerificationCandidate
    pub async fn classify(
        &self,
        image_data: &[u8],
        detections: &[CvDetection],
        config: &DetectionConfig,
    ) -> Result<Vec<CvEntity>> {
        let agent = self.agent.as_ref().ok_or_else(|| {
            Error::new(
                ErrorKind::Validation,
                "CvPipeline::classify called on a verifier-only pipeline",
            )
        })?;
        agent.classify(image_data, detections, config).await
    }

    /// Verify upstream entity proposals against the image.
    ///
    /// Confirmed entities pass through unchanged; corrected
    /// entities are updated with the verifier's edits; rejected
    /// entities are dropped. Empty `candidates` returns an empty
    /// list without prompting the LLM.
    pub async fn verify(
        &self,
        image_data: &Bytes,
        candidates: Vec<VerificationCandidate>,
    ) -> Result<Vec<Entity>> {
        self.verifier.verify_entities(image_data, candidates).await
    }
}

impl CvPipeline {
    /// Reset cumulative usage counters. No cross-call coreference
    /// state, but per-document accounting still wants zeroing at
    /// document boundaries.
    pub async fn reset(&self) {
        if let Some(ref agent) = self.agent {
            agent.tracker().reset();
        }
        self.verifier.tracker().reset();
    }

    /// Cumulative token usage since the last [`reset`], summed
    /// across the optional classification agent and the verifier
    /// agent.
    ///
    /// [`reset`]: Self::reset
    pub fn usage(&self) -> UsageStats {
        let mut stats = UsageStats::default();
        if let Some(ref agent) = self.agent {
            stats += agent.tracker().snapshot();
        }
        stats += self.verifier.tracker().snapshot();
        stats
    }
}
