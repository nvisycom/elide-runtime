//! [`VlmExtractor`]: VLM-backed entity verifier for image documents.
//!
//! Built once at engine startup from [`VlmExtractorConfig`] and
//! shared across every run via [`Extractors`]. Runs an LLM pass
//! over already-detected entities, confirming/correcting them
//! against the source image.
//!
//! Today the VLM extractor is a *verifier* only — it consumes the
//! entity list produced by detection upstream and refines it.
//! Future work: standalone CV entity detection (no prior NER).
//!
//! [`Extractors`]: super::Extractors

mod params;

use bytes::Bytes;
use nvisy_agent::agent::cv::VerificationCandidate;
use nvisy_agent::pipeline::CvPipeline;
use nvisy_codec::core::Located;
use nvisy_codec::handler::ImageData;
use nvisy_core::{Error, Result};
use nvisy_ontology::entity::Entity;
use nvisy_ontology::modality::Image;

pub use self::params::VlmExtractorConfig;
use crate::envelope::DocumentEnvelope;
use crate::envelope::value_at::ValueAt;

const TARGET: &str = "nvisy_engine::extraction::vlm";

/// Pre-built VLM extractor: CV pipeline wrapping an LLM agent.
pub struct VlmExtractor {
    pipeline: CvPipeline,
}

impl VlmExtractor {
    /// Build from a [`VlmExtractorConfig`] bundle.
    ///
    /// # Errors
    ///
    /// Returns an error if the CV pipeline cannot be constructed.
    pub fn from_config(cfg: VlmExtractorConfig) -> Result<Self> {
        let pipeline = CvPipeline::new(&cfg.provider, None, cfg.agent)
            .map_err(|e| Error::runtime(e.to_string(), "cv-pipeline", false))?;
        Ok(Self { pipeline })
    }

    /// Run the VLM verifier over the envelope's detected image
    /// entities, refining them against the source image.
    ///
    /// Skips when there are no entities or no image regions.
    /// Failures are logged but not propagated — the unverified
    /// entities are kept rather than failing the run.
    pub async fn run(&self, envelope: &mut DocumentEnvelope<Image>) -> Result<()> {
        if envelope.document.audit.entities.is_empty() {
            return Ok(());
        }
        let inputs = Self::collect_inputs(envelope).await;
        if inputs.is_empty() {
            return Ok(());
        }

        let image_entities = std::mem::take(&mut envelope.document.audit.entities);

        tracing::debug!(
            target: TARGET,
            entities = image_entities.len(),
            regions = inputs.len(),
            "running VLM verification",
        );

        match self.verify(&inputs, image_entities, envelope).await {
            Ok(verified) => envelope.document.audit.entities = verified,
            Err(e) => tracing::warn!(
                target: TARGET, error = %e,
                "VLM verification failed, keeping unverified entities"
            ),
        }
        Ok(())
    }

    async fn verify(
        &self,
        inputs: &[Located<Image, ImageData>],
        entities: Vec<Entity<Image>>,
        envelope: &DocumentEnvelope<Image>,
    ) -> Result<Vec<Entity<Image>>> {
        let mut verified = entities;
        for item in inputs {
            if verified.is_empty() {
                break;
            }
            let png_bytes: Bytes = item.data.encode_png()?;

            let mut candidates = Vec::with_capacity(verified.len());
            for entity in verified {
                let value = envelope
                    .value_at(&entity.location)
                    .await
                    .unwrap_or_default();
                candidates.push(VerificationCandidate { entity, value });
            }

            verified = self
                .pipeline
                .verify(&png_bytes, candidates)
                .await
                .map_err(|e| Error::runtime(e.to_string(), "cv-pipeline", e.is_retryable()))?;
        }
        Ok(verified)
    }

    async fn collect_inputs(
        envelope: &DocumentEnvelope<Image>,
    ) -> Vec<Located<Image, ImageData>> {
        let locations = envelope.collect_image_locations().await;
        let mut out = Vec::with_capacity(locations.len());
        for located in locations {
            if let Some(data) = envelope.read_image(&located.location).await {
                out.push(located.with_data(data));
            }
        }
        out
    }
}
