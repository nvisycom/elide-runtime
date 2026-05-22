//! Computer-vision classification agent.
//!
//! [`CvAgent`] wraps a [`BaseAgent`] with a classification-only
//! prompt: it takes pre-computed [`CvDetection`]s (produced
//! upstream by a face/plate/signature detector) and asks the VLM to
//! classify each one into an entity category and type.
//!
//! Detection itself does not live in this crate — the caller runs
//! their CV backend and passes the bboxes in. This keeps rig free
//! of model/inference dependencies.
//!
//! [`BaseAgent`]: super::BaseAgent

mod output;
mod prompt;

use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use nvisy_core::Result;
use serde::Serialize;
use uuid::Uuid;

pub(crate) use self::output::CvEntities;
pub use self::output::CvEntity;
use self::prompt::{CV_SYSTEM_PROMPT, CvPromptBuilder};
use super::base::UsageTracker;
use super::{AgentConfig, AgentProvider, BaseAgent, DetectionConfig};

const TARGET: &str = "nvisy_rig::agent::cv";

/// A single computer-vision detection produced by an upstream CV
/// backend (face detector, plate detector, etc.).
///
/// This is the input shape for [`CvAgent::classify`]: the agent
/// receives pre-computed bboxes and labels and decides what entity
/// each one corresponds to. The detector lives outside `nvisy-rig`.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct CvDetection {
    /// Label for the detected object (e.g. `"face"`, `"license_plate"`).
    pub label: String,
    /// Detection confidence in the range `0.0..=1.0`.
    pub confidence: f64,
    /// Bounding box as `[x, y, width, height]` in pixels.
    pub bbox: [f64; 4],
}

/// VLM agent that classifies pre-computed CV detections into entity
/// categories.
///
/// # Workflow
///
/// 1. Caller (or pipeline) runs a CV backend to produce
///    [`CvDetection`]s with bboxes.
/// 2. Caller passes the image and detections to [`classify`].
/// 3. The agent encodes the image as base64, embeds the detections
///    as JSON in the prompt, and asks the VLM to classify each.
/// 4. Structured output is parsed into `Vec<CvEntity>`.
///
/// [`classify`]: Self::classify
pub struct CvAgent {
    base: BaseAgent,
}

impl CvAgent {
    /// Create a new CV classification agent.
    pub fn new(provider: &AgentProvider, mut config: AgentConfig) -> Result<Self> {
        config
            .preamble
            .get_or_insert_with(|| CV_SYSTEM_PROMPT.into());
        let base = BaseAgent::builder(provider, config)
            .build()
            .map_err(crate::error::convert)?;
        Ok(Self { base })
    }

    /// Unique identifier for this agent instance (UUIDv7).
    pub fn id(&self) -> Uuid {
        self.base.id()
    }

    /// Configured model name.
    pub fn model_name(&self) -> &str {
        self.base.model_name()
    }

    /// Access the usage tracker for this agent's LLM calls.
    pub fn tracker(&self) -> &UsageTracker {
        self.base.tracker()
    }

    /// Classify pre-computed CV detections into entity categories.
    ///
    /// `detections` come from an upstream CV backend. Returns the
    /// detections as classified entities; an empty input returns an
    /// empty output without prompting the LLM.
    #[tracing::instrument(
        target = "nvisy_rig::agent::cv",
        skip_all,
        fields(image_bytes = image_data.len(), detection_count = detections.len()),
    )]
    pub async fn classify(
        &self,
        image_data: &[u8],
        detections: &[CvDetection],
        config: &DetectionConfig,
    ) -> Result<Vec<CvEntity>> {
        if detections.is_empty() {
            return Ok(Vec::new());
        }

        let image_b64 = STANDARD.encode(image_data);
        tracing::debug!(
            target: TARGET,
            b64_len = image_b64.len(),
            entity_kinds = config.entity_kinds.len(),
            "encoded image, building classification prompt"
        );

        let prompt = CvPromptBuilder::new(config, detections).build(&image_b64);

        let result: CvEntities = self
            .base
            .prompt_structured_raw(&prompt)
            .await
            .map_err(crate::error::convert)?;

        tracing::info!(
            target: TARGET,
            entity_count = result.entities.len(),
            "cv classification complete"
        );

        Ok(result.entities)
    }
}
