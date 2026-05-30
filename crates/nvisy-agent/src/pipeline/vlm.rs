//! [`VlmPipeline`]: optional VLM detect + optional VLM verify.
//!
//! Bundles a [`VlmAgent`] (direct image entity discovery) and a
//! [`VlmVerifyAgent`] (validates entity proposals against the
//! image). Both flows are exposed as independent methods; callers
//! that only need one configure the other as `None`.
//!
//! Stateless across calls; [`reset`] only zeroes usage trackers.
//!
//! [`VlmAgent`]: crate::agent::vlm::VlmAgent
//! [`VlmVerifyAgent`]: crate::agent::vlm::VlmVerifyAgent
//! [`reset`]: VlmPipeline::reset

use bytes::Bytes;
use nvisy_core::{Error, Result};
use nvisy_ontology::entity::Entity;
use nvisy_ontology::modality::Image;
use nvisy_ontology::primitive::Dimensions;

use crate::agent::vlm::{VerificationCandidate, VlmAgent, VlmVerifyAgent};
use crate::agent::{AgentConfig, AgentProvider, UsageStats, VlmDetectContext};

/// Combined VLM pipeline.
pub struct VlmPipeline {
    agent: Option<VlmAgent>,
    verifier: Option<VlmVerifyAgent>,
}

impl VlmPipeline {
    /// Build a pipeline from an LLM provider plus optional agent
    /// configs.
    ///
    /// `agent_config` drives the detect agent when `Some`;
    /// `verifier_config` drives the verifier when `Some`. At least
    /// one must be `Some` — a pipeline with neither would never
    /// produce or process entities.
    ///
    /// # Errors
    ///
    /// Returns an error if both configs are `None`, or if any
    /// requested agent cannot be constructed.
    pub fn new(
        provider: &AgentProvider,
        agent_config: Option<AgentConfig>,
        verifier_config: Option<AgentConfig>,
    ) -> Result<Self> {
        if agent_config.is_none() && verifier_config.is_none() {
            return Err(Error::validation(
                "VlmPipeline requires at least one of detect or verify to be configured",
                "vlm-pipeline",
            ));
        }
        let agent = match agent_config {
            Some(cfg) => Some(VlmAgent::new(provider, cfg)?),
            None => None,
        };
        let verifier = match verifier_config {
            Some(cfg) => Some(VlmVerifyAgent::new(provider, cfg)?),
            None => None,
        };
        Ok(Self { agent, verifier })
    }

    /// Run the VLM detect pass over an image.
    ///
    /// # Errors
    ///
    /// Returns an error if this pipeline was constructed without
    /// an `agent_config` (verify-only mode), or if the VLM call
    /// fails.
    pub async fn detect(
        &self,
        image_data: &Bytes,
        dims: Dimensions,
        config: &VlmDetectContext,
    ) -> Result<Vec<Entity<Image>>> {
        let agent = self.agent.as_ref().ok_or_else(|| {
            Error::validation(
                "VlmPipeline::detect called on a verifier-only pipeline",
                "vlm-pipeline",
            )
        })?;
        agent.detect(image_data, dims, config).await
    }

    /// Verify upstream entity proposals against the image.
    ///
    /// # Errors
    ///
    /// Returns an error if this pipeline was constructed without
    /// a `verifier_config` (detect-only mode), or if the VLM call
    /// fails.
    pub async fn verify(
        &self,
        image_data: &Bytes,
        candidates: Vec<VerificationCandidate>,
    ) -> Result<Vec<Entity<Image>>> {
        let verifier = self.verifier.as_ref().ok_or_else(|| {
            Error::validation(
                "VlmPipeline::verify called on a detect-only pipeline",
                "vlm-pipeline",
            )
        })?;
        verifier.verify_entities(image_data, candidates).await
    }

    /// Reset cumulative usage counters. Per-document accounting
    /// wants zeroing at document boundaries.
    pub async fn reset(&self) {
        if let Some(a) = &self.agent {
            a.tracker().reset();
        }
        if let Some(v) = &self.verifier {
            v.tracker().reset();
        }
    }

    /// Cumulative token usage since the last [`reset`], summed
    /// across the configured agents.
    ///
    /// [`reset`]: Self::reset
    pub fn usage(&self) -> UsageStats {
        let mut stats = UsageStats::default();
        if let Some(a) = &self.agent {
            stats += a.tracker().snapshot();
        }
        if let Some(v) = &self.verifier {
            stats += v.tracker().snapshot();
        }
        stats
    }
}
