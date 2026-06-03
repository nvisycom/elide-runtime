//! Image-side LLM entity detection via a vision-language model.
//!
//! [`VlmAgent`] implements [`EntityRecognizer<Image>`]. Each
//! [`recognize`] call passes the image bytes + pixel dimensions
//! (plus any uploader-supplied [`Hint<Image>`]s and document
//! labels) to a VLM, which draws bounding boxes around every
//! sensitive entity it sees. Output is ready-to-use
//! [`Entity<Image>`] values whose trail starts with a recognition
//! step carrying the VLM's model provenance.
//!
//! [`Entity<Image>`]: nvisy_core::entity::Entity
//! [`EntityRecognizer<Image>`]: nvisy_core::EntityRecognizer
//! [`Hint<Image>`]: nvisy_core::Hint
//! [`recognize`]: EntityRecognizer::recognize

mod output;
mod prompt;

use async_trait::async_trait;
use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use nvisy_core::entity::{Entity, ModelProvenance, TrailProvenance, TrailStep};
use nvisy_core::modality::Image;
use nvisy_core::primitive::{Confidence, Dimensions};
use nvisy_core::{EntityRecognizer, RecognizerInput, RecognizerOutput, Result};
use uuid::Uuid;

use self::output::{VlmDetectedEntities, VlmDetectedEntity};
use self::prompt::{VLM_DETECT_SYSTEM_PROMPT, VlmDetectPromptBuilder};
use crate::agent::base::{BaseAgent, UsageTracker};
use crate::agent::{AgentConfig, AgentProvider};

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

#[async_trait]
impl EntityRecognizer<Image> for VlmAgent {
    /// Detect image entities directly via the VLM.
    ///
    /// The VLM emits normalised `[0, 1]` bounding boxes; we scale
    /// them to pixel coordinates using the input's `dims` before
    /// constructing [`Entity<Image>`].
    ///
    /// [`Entity<Image>`]: nvisy_core::entity::Entity
    #[tracing::instrument(
        target = TARGET,
        skip_all,
        fields(
            image_bytes = input.data.bytes.len(),
            width = input.data.dims.width,
            height = input.data.dims.height,
            hint_count = input.hints.len(),
            label_count = input.labels.len(),
        ),
    )]
    async fn recognize(&self, input: &RecognizerInput<Image>) -> Result<RecognizerOutput<Image>> {
        let dims = input.data.dims;
        let image_b64 = STANDARD.encode(input.data.bytes.as_ref());
        let prompt = VlmDetectPromptBuilder::new(&input.hints, &input.labels).build(&image_b64);

        tracing::debug!(
            target: TARGET,
            b64_len = image_b64.len(),
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

        Ok(RecognizerOutput::new(entities))
    }

    async fn reset(&self) {
        self.base.tracker().reset();
    }
}
