//! OCR verification of detected entities against source images.
//!
//! Takes entities detected by prior operations (NER, pattern match, etc.)
//! and verifies them against the original image using a VLM. Entities may
//! be confirmed, corrected, or rejected.
//!
//! [`OcrAgent`]: nvisy_rig::agent::OcrAgent

use nvisy_codec::Span;
use nvisy_codec::handler::ImageData;
use nvisy_core::{Error, Result};
use nvisy_ontology::entity::Entity;
use nvisy_rig::agent::OcrAgent;

use crate::operation::{Operation, ParallelContext};

const TARGET: &str = "nvisy_engine::op::ocr_verification";

/// Input for the OCR verification operation.
pub struct OcrVerificationInput {
    /// Image spans to verify against.
    pub image_spans: Vec<Span<(), ImageData>>,
    /// Entities detected by prior operations that should be verified.
    pub entities: Vec<Entity>,
}

/// OCR verification operation: verifies detected entities against images
/// via VLM, correcting or rejecting false positives.
///
/// [`OcrAgent`]: nvisy_rig::agent::OcrAgent
pub struct OcrVerification {
    agent: OcrAgent,
}

impl OcrVerification {
    /// Create a new OCR verification operation from a pre-built agent.
    pub fn new(agent: OcrAgent) -> Self {
        Self { agent }
    }
}

impl OcrVerification {
    async fn verify(&self, data: OcrVerificationInput) -> Result<Vec<Entity>> {
        if data.entities.is_empty() {
            tracing::debug!(target: TARGET, "no entities to verify");
            return Ok(Vec::new());
        }
        tracing::debug!(target: TARGET, entity_count = data.entities.len(), "verifying entities");

        let image_bytes = match data.image_spans.first() {
            Some(span) => span.data.encode_png()?,
            None => return Ok(data.entities),
        };

        let entities = self
            .agent
            .verify_entities(&image_bytes, data.entities)
            .await
            .map_err(|e| Error::runtime(e.to_string(), "ocr-verification", e.is_retryable()))?;

        Ok(entities)
    }
}

impl Operation for OcrVerification {
    type Input = ParallelContext<OcrVerificationInput>;
    type Output = ParallelContext<Vec<Entity>>;

    async fn call(&self, input: Self::Input) -> Result<Self::Output> {
        input.parallel_map(|data| self.verify(data)).await
    }
}
