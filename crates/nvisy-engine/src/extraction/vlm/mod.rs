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
use nvisy_codec::Span;
use nvisy_codec::handler::ImageData;
use nvisy_core::{Error, Result};
use nvisy_ontology::entity::{Entities, Entity};
use nvisy_ontology::modality::{AnyModality, Image};

pub use self::params::VlmExtractorConfig;
use crate::envelope::DocumentEnvelope;

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
    /// Skips when there are no entities or no image spans. Failures
    /// are logged but not propagated — the unverified entities are
    /// kept rather than failing the run.
    pub async fn run(&self, envelope: &mut DocumentEnvelope) -> Result<()> {
        if envelope.audit.entities.is_empty() {
            return Ok(());
        }
        let spans = Self::collect_spans(envelope).await;
        if spans.is_empty() {
            return Ok(());
        }

        // Partition audit entities into image-modality (verifiable)
        // and others (passed through unchanged).
        let (image_entities, other_entities): (Vec<_>, Vec<_>) = envelope
            .audit
            .entities
            .0
            .iter()
            .cloned()
            .partition(|e| matches!(e.location, AnyModality::Image(_)));

        let image_entities: Vec<Entity<Image>> = image_entities
            .into_iter()
            .filter_map(|e| match e.location.clone() {
                AnyModality::Image(loc) => Some(rebuild_as_image(e, loc)),
                _ => None,
            })
            .collect();

        if image_entities.is_empty() {
            return Ok(());
        }

        tracing::debug!(
            target: TARGET,
            entities = image_entities.len(),
            spans = spans.len(),
            "running VLM verification",
        );

        match self
            .verify(&spans, image_entities, envelope)
            .await
        {
            Ok(verified) => {
                let mut merged: Vec<Entity<AnyModality>> = other_entities;
                merged.extend(verified.into_iter().map(Entity::erase));
                envelope.audit.entities = Entities::from(merged);
            }
            Err(e) => tracing::warn!(
                target: TARGET, error = %e,
                "VLM verification failed, keeping unverified entities"
            ),
        }
        Ok(())
    }

    async fn verify(
        &self,
        spans: &[Span<Image, ImageData>],
        entities: Vec<Entity<Image>>,
        envelope: &DocumentEnvelope,
    ) -> Result<Vec<Entity<Image>>> {
        let mut verified = entities;
        for span in spans {
            if verified.is_empty() {
                break;
            }
            let png_bytes: Bytes = span.data.encode_png()?;

            let mut candidates = Vec::with_capacity(verified.len());
            for entity in verified {
                let location = AnyModality::Image(entity.location.clone());
                let value = envelope.value_at(&location).await.unwrap_or_default();
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

    async fn collect_spans(envelope: &DocumentEnvelope) -> Vec<Span<Image, ImageData>> {
        let locations = envelope.collect_image_locations().await;
        let mut spans = Vec::with_capacity(locations.len());
        for located in locations {
            if let Some(data) = envelope.read_image(&located.location).await {
                spans.push(Span::from_located(located, data));
            }
        }
        spans
    }
}

/// Rebuild an [`Entity<AnyModality>`] as [`Entity<Image>`] given a
/// known-Image location component. All non-location fields pass
/// through verbatim.
fn rebuild_as_image(e: Entity<AnyModality>, location: Image) -> Entity<Image> {
    Entity {
        id: e.id,
        entity_id: e.entity_id,
        category: e.category,
        entity_kind: e.entity_kind,
        extraction_methods: e.extraction_methods,
        recognition_methods: e.recognition_methods,
        refinement_methods: e.refinement_methods,
        confidence: e.confidence,
        location,
        language: e.language,
        sensitivity: e.sensitivity,
    }
}
