//! OCR agent for vision-based text extraction and entity detection.
//!
//! [`OcrAgent`] wraps a [`BaseAgent`](crate::backend::BaseAgent) with an
//! [`OcrProvider`]-backed tool. It encodes an image as base64, prompts the
//! VLM to call the OCR tool, and returns extracted text together with any
//! entities found in it.

mod output;
mod prompt;
mod tool;

pub use output::{OcrOutput, OcrEntity};

use async_trait::async_trait;
use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use serde::Serialize;
use uuid::Uuid;

use nvisy_core::math::{BoundingBox, Polygon};
use nvisy_ontology::location::TextLevel;

use crate::backend::UsageTracker;
use super::{AgentProvider, DetectionConfig};
use super::{BaseAgent, AgentConfig};
use crate::error::Error;
use prompt::{OCR_SYSTEM_PROMPT, OcrPromptBuilder};
use tool::OcrRigTool;

/// A single text region extracted by an OCR provider.
///
/// Each region represents a contiguous block of text found in the image,
/// together with an optional bounding box and confidence score.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct OcrTextRegion {
    /// The extracted text content.
    pub text: String,
    /// Confidence of the OCR extraction (0.0..=1.0).
    pub confidence: f64,
    /// Axis-aligned bounding box (always present from traditional OCR).
    pub bbox: Option<BoundingBox>,
    /// Polygon vertices for rotated text regions.
    pub polygon: Option<Polygon>,
    /// Hierarchical level of this text region.
    pub level: Option<TextLevel>,
}

/// Trait for OCR capabilities that can be provided to VLM agents.
///
/// Consumers implement this trait to supply text extraction from images.
/// The trait is intentionally free of rig-core types so it can be
/// implemented in any crate without pulling in the LLM framework.
///
/// Implementations return a list of [`OcrTextRegion`]s, each carrying the
/// extracted text, a confidence score, and an optional pixel-space bounding
/// box. Returning multiple regions allows the downstream VLM to reason
/// about spatial layout (e.g. headers vs body text, table cells).
#[async_trait]
pub trait OcrProvider: Send + Sync {
    /// Extract text regions from raw image bytes (PNG, JPEG, etc.).
    async fn extract_text(&self, image_data: &[u8]) -> Result<Vec<OcrTextRegion>, Error>;
}

/// VLM agent that extracts text from images and detects entities in it.
///
/// # Workflow
///
/// 1. Caller passes raw image bytes to [`extract_and_detect`](Self::extract_and_detect).
/// 2. The agent base64-encodes the image and builds a user prompt via
///    [`OcrPromptBuilder`].
/// 3. The VLM is instructed to call the `ocr_extract_text` tool (backed by
///    the [`OcrProvider`]) and then analyse the result for PII/PHI entities.
/// 4. Structured output is parsed into [`OcrOutput`].
pub struct OcrAgent {
    base: BaseAgent,
}

impl OcrAgent {
    /// Create a new OCR agent.
    pub fn new(
        provider: &AgentProvider,
        mut config: AgentConfig,
        ocr: impl OcrProvider + 'static,
    ) -> Result<Self, Error> {
        config.preamble.get_or_insert_with(|| OCR_SYSTEM_PROMPT.into());
        let base = BaseAgent::builder(provider, config)
            .tool(OcrRigTool::new(ocr))
            .build()?;
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

    /// Extract text from an image and detect entities in the extracted text.
    #[tracing::instrument(
        skip_all,
        fields(image_bytes = image_data.len(), agent = "ocr"),
    )]
    pub async fn extract_and_detect(
        &self,
        image_data: &[u8],
        config: &DetectionConfig,
    ) -> Result<OcrOutput, Error> {
        let image_b64 = STANDARD.encode(image_data);
        tracing::debug!(
            b64_len = image_b64.len(),
            entity_kinds = config.entity_kinds.len(),
            "encoded image, building prompt"
        );

        let prompt = OcrPromptBuilder::new(config).build(&image_b64);

        let output: OcrOutput = self.base.prompt_structured(&prompt).await?;

        tracing::info!(
            text_len = output.extracted_text.len(),
            entity_count = output.entities.len(),
            "ocr extraction complete"
        );

        Ok(output)
    }
}
