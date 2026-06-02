//! Image-side LLM entity detection via a vision-language model.
//!
//! [`VlmAgent`] takes an image (raw bytes + [`Dimensions`]) plus a
//! [`VlmDetectContext`] and asks the VLM to draw bounding boxes
//! around every sensitive entity it sees. Output is ready-to-use
//! [`Entity<Image>`] values whose trail starts with a recognition
//! step carrying the VLM's model provenance.
//!
//! [`Dimensions`]: nvisy_core::primitive::Dimensions
//! [`Entity<Image>`]: nvisy_core::entity::Entity

mod output;
mod prompt;

use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use bytes::Bytes;
use nvisy_core::Result;
use nvisy_core::entity::{Entity, ModelProvenance, TrailProvenance, TrailStep};
use nvisy_core::modality::Image;
use nvisy_core::primitive::{Confidence, Dimensions};
use uuid::Uuid;

use self::output::{VlmDetectedEntities, VlmDetectedEntity};
use self::prompt::{VLM_DETECT_SYSTEM_PROMPT, VlmDetectPromptBuilder};
use crate::agent::base::{BaseAgent, UsageTracker};
use crate::agent::{AgentConfig, AgentProvider, VlmDetectContext};

const TARGET: &str = "nvisy_agent::agent::vlm::detect";

/// Default confidence assigned to a detection when the VLM didn't
/// score it.
const DEFAULT_CONFIDENCE: f64 = 0.5;

/// VLM-driven image entity detector.
pub struct VlmAgent {
    base: BaseAgent,
}

impl VlmAgent {
    /// Construct a detect agent from an LLM provider + agent config.
    ///
    /// The config's preamble defaults to the built-in VLM detect
    /// system prompt when unset.
    pub fn new(provider: &AgentProvider, mut config: AgentConfig) -> Result<Self> {
        config
            .preamble
            .get_or_insert_with(|| VLM_DETECT_SYSTEM_PROMPT.into());
        let base = BaseAgent::builder(provider, config)
            .build()
            .map_err(crate::error::convert)?;
        Ok(Self { base })
    }

    /// UUID of this agent instance (UUIDv7).
    pub fn id(&self) -> Uuid {
        self.base.id()
    }

    /// Configured model name.
    pub fn model_name(&self) -> &str {
        self.base.model_name()
    }

    /// Usage tracker for this agent's LLM calls.
    pub fn tracker(&self) -> &UsageTracker {
        self.base.tracker()
    }

    /// Detect image entities directly via the VLM.
    ///
    /// The VLM emits normalised `[0, 1]` bounding boxes; we scale
    /// them to pixel coordinates using `dims` before constructing
    /// [`Entity<Image>`].
    ///
    /// [`Entity<Image>`]: nvisy_core::entity::Entity
    #[tracing::instrument(
        target = TARGET,
        skip_all,
        fields(image_bytes = image_data.len(), width = dims.width, height = dims.height),
    )]
    pub async fn detect(
        &self,
        image_data: &Bytes,
        dims: Dimensions,
        config: &VlmDetectContext,
    ) -> Result<Vec<Entity<Image>>> {
        let image_b64 = STANDARD.encode(image_data);
        let prompt = VlmDetectPromptBuilder::new(config).build(&image_b64);

        tracing::debug!(
            target: TARGET,
            b64_len = image_b64.len(),
            entity_kinds = config.entity_kinds.len(),
            "built VLM detect prompt"
        );

        let result: VlmDetectedEntities = self
            .base
            .prompt_structured_raw(&prompt)
            .await
            .map_err(crate::error::convert)?;

        let model = ModelProvenance::new(self.base.model_name().to_owned());
        let entities = self.build_entities(result.entities, dims, &model);

        tracing::info!(
            target: TARGET,
            entity_count = entities.len(),
            "VLM detect complete"
        );

        Ok(entities)
    }

    /// Lift the VLM's normalised detections into pixel-space
    /// [`Entity<Image>`]s. Drops detections whose confidence
    /// falls outside `[0, 1]` after clamping.
    fn build_entities(
        &self,
        detections: Vec<VlmDetectedEntity>,
        dims: Dimensions,
        model: &ModelProvenance,
    ) -> Vec<Entity<Image>> {
        let mut out = Vec::with_capacity(detections.len());
        let mut dropped_bad_confidence = 0usize;

        for d in detections {
            let raw = d.confidence.unwrap_or(DEFAULT_CONFIDENCE);
            let Some(confidence) = Confidence::new(raw.clamp(0.0, 1.0)) else {
                dropped_bad_confidence += 1;
                continue;
            };
            let bbox = d.bbox.to_pixel(dims);
            let location = Image::new(bbox);
            let provenance = TrailProvenance::Model(model.clone());
            let reason = format!("vlm '{}' identified {}", model.name, d.entity_kind);
            let step = TrailStep::recognition("llm-vlm", confidence, provenance, reason);

            let entity = Entity::builder()
                .with_entity_kind(d.entity_kind)
                .with_trail(vec![step])
                .with_confidence(confidence)
                .with_location(location)
                .build()
                .expect("required fields provided");
            out.push(entity);
        }

        if dropped_bad_confidence > 0 {
            tracing::debug!(
                target: TARGET,
                dropped_bad_confidence,
                "dropped detections during entity construction"
            );
        }
        out
    }
}
