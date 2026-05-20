//! Computer vision agent for face, license plate, and signature detection.
//!
//! [`CvAgent`] wraps a [`BaseAgent`] with a
//! [`CvProvider`]-backed tool. It encodes an image as base64, prompts the
//! VLM to call the CV tool, and returns classified entities with bounding
//! boxes.
//!
//! [`BaseAgent`]: crate::backend::BaseAgent

mod output;
mod prompt;
mod tool;

use async_trait::async_trait;
use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use nvisy_core::Result;
use serde::Serialize;
use uuid::Uuid;

pub use self::output::{CvEntities, CvEntity};
use self::prompt::{CV_SYSTEM_PROMPT, CvPromptBuilder};
use self::tool::CvRigTool;
use super::base::UsageTracker;
use super::{AgentConfig, AgentProvider, BaseAgent, DetectionConfig};

const TARGET: &str = "nvisy_rig::agent::cv";

/// A single computer-vision detection result returned by a [`CvProvider`].
///
/// This is the raw output from the CV backend before the VLM classifies
/// detections into entity categories. It carries a human-readable label,
/// a confidence score, and a pixel-space bounding box.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct CvDetection {
    /// Label for the detected object (e.g. `"face"`, `"license_plate"`).
    pub label: String,
    /// Detection confidence in the range `0.0..=1.0`.
    pub confidence: f64,
    /// Bounding box as `[x, y, width, height]` in pixels.
    pub bbox: [f64; 4],
}

/// Trait for computer-vision capabilities (face/plate/signature detection).
///
/// Consumers implement this trait to supply object detection from images.
/// The trait is intentionally free of rig-core types so it can be
/// implemented in any crate without pulling in the LLM framework.
#[async_trait]
pub trait CvProvider: Send + Sync {
    /// Detect objects in raw image bytes (PNG, JPEG, etc.).
    async fn detect_objects(&self, image_data: &[u8]) -> Result<Vec<CvDetection>>;
}

/// VLM agent that detects privacy-sensitive objects in images.
///
/// # Workflow
///
/// 1. Caller passes raw image bytes to [`detect`].
/// 2. The agent base64-encodes the image and builds a user prompt via
///    `CvPromptBuilder`.
/// 3. The VLM is instructed to call the `cv_detect_objects` tool (backed
///    by the [`CvProvider`]) and then classify each detection into an
///    entity category and type.
/// 4. Structured output is parsed into a `Vec<CvEntity>`.
///
/// [`detect`]: Self::detect
pub struct CvAgent {
    base: BaseAgent,
}

impl CvAgent {
    /// Create a new CV agent.
    pub fn new(
        provider: &AgentProvider,
        mut config: AgentConfig,
        cv: impl CvProvider + 'static,
    ) -> Result<Self> {
        config
            .preamble
            .get_or_insert_with(|| CV_SYSTEM_PROMPT.into());
        let base = BaseAgent::builder(provider, config)
            .tool(CvRigTool::new(cv))
            .build()
            .map_err(crate::error::convert)?;
        Ok(Self { base })
    }

    /// Unique identifier for this agent instance (UUIDv7).
    pub fn id(&self) -> Uuid {
        self.base.id()
    }

    /// Access the usage tracker for this agent's LLM calls.
    pub fn tracker(&self) -> &UsageTracker {
        self.base.tracker()
    }

    /// Detect privacy-sensitive objects in an image.
    #[tracing::instrument(
        target = "nvisy_rig::agent::cv",
        skip_all,
        fields(image_bytes = image_data.len()),
    )]
    pub async fn detect(
        &self,
        image_data: &[u8],
        config: &DetectionConfig,
    ) -> Result<Vec<CvEntity>> {
        let image_b64 = STANDARD.encode(image_data);
        tracing::debug!(
            target: TARGET,
            b64_len = image_b64.len(),
            entity_kinds = config.entity_kinds.len(),
            "encoded image, building prompt"
        );

        let prompt = CvPromptBuilder::new(config).build(&image_b64);

        let result: CvEntities = self
            .base
            .prompt_structured_raw(&prompt)
            .await
            .map_err(crate::error::convert)?;

        tracing::info!(
            target: TARGET,
            entity_count = result.entities.len(),
            "cv detection complete"
        );

        Ok(result.entities)
    }
}
