//! Computer vision agent for face, license plate, and signature detection.
//!
//! [`CvAgent`] wraps a [`BaseAgent`](super::BaseAgent) with a
//! [`CvProvider`]-backed tool. It encodes an image as base64, prompts the
//! VLM to call the CV tool, and returns classified entities with bounding
//! boxes.

mod output;
mod prompt;
mod tool;

pub use output::{RawCvEntities, RawCvEntity};

use std::sync::Arc;

use async_trait::async_trait;
use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use rig::completion::CompletionModel;
use serde::Serialize;

use nvisy_core::Error;

use crate::backend::{DetectionConfig, UsageTracker};

use super::base::{BaseAgent, BaseAgentConfig};
use prompt::{CvPromptBuilder, CV_SYSTEM_PROMPT};
use tool::CvRigTool;

/// A single computer-vision detection result returned by a [`CvProvider`].
///
/// This is the raw output from the CV backend before the VLM classifies
/// detections into entity categories. It carries a human-readable label,
/// a confidence score, and a pixel-space bounding box.
#[derive(Debug, Clone, Serialize)]
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
    async fn detect_objects(&self, image_data: &[u8]) -> Result<Vec<CvDetection>, Error>;
}

/// VLM agent that detects privacy-sensitive objects in images.
///
/// # Workflow
///
/// 1. Caller passes raw image bytes to [`detect`](Self::detect).
/// 2. The agent base64-encodes the image and builds a user prompt via
///    [`CvPromptBuilder`].
/// 3. The VLM is instructed to call the `cv_detect_objects` tool (backed
///    by the [`CvProvider`]) and then classify each detection into an
///    entity category and type.
/// 4. Structured output is parsed into a `Vec<RawCvEntity>`.
pub struct CvAgent<M: CompletionModel> {
    base: BaseAgent<M>,
}

impl<M: CompletionModel> CvAgent<M> {
    /// Create a new CV agent with the given model, config, and CV provider.
    pub fn new(model: M, config: BaseAgentConfig, cv: impl CvProvider + 'static) -> Self {
        let base = BaseAgent::builder(model, config)
            .preamble(CV_SYSTEM_PROMPT)
            .tool(CvRigTool(Arc::new(cv)))
            .build();
        Self { base }
    }

    /// Access the usage tracker for this agent's LLM calls.
    pub fn tracker(&self) -> &UsageTracker {
        self.base.tracker()
    }

    /// Detect privacy-sensitive objects in an image.
    #[tracing::instrument(
        skip_all,
        fields(image_bytes = image_data.len(), agent = "cv"),
    )]
    pub async fn detect(
        &self,
        image_data: &[u8],
        config: &DetectionConfig,
    ) -> Result<Vec<RawCvEntity>, Error> {
        let image_b64 = STANDARD.encode(image_data);
        tracing::debug!(
            b64_len = image_b64.len(),
            entity_kinds = config.entity_kinds.len(),
            "encoded image, building prompt"
        );

        let prompt = CvPromptBuilder::new(config).build(&image_b64);

        let result: RawCvEntities = self
            .base
            .prompt_structured(&prompt, config.system_prompt.as_deref())
            .await?;

        tracing::info!(
            entity_count = result.entities.len(),
            "cv detection complete"
        );

        Ok(result.entities)
    }
}
