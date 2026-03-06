//! OCR verification of detected entities against source images.
//!
//! Takes entities detected by prior operations (NER, pattern match, etc.)
//! and verifies them against the original image using a VLM. Entities may
//! be confirmed, corrected, or rejected.
//!
//! [`OcrAgent`]: nvisy_rig::agent::OcrAgent

use nvisy_codec::document::Span;
use nvisy_codec::handler::ImageData;
use nvisy_core::Error;
use nvisy_ontology::entity::Entity;
use nvisy_rig::agent::OcrAgent;

use crate::operation::{Operation, ParallelContext};

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

impl Operation for OcrVerification {
    type Input = ParallelContext<OcrVerificationInput>;
    type Output = ParallelContext<Vec<Entity>>;

    async fn call(&self, input: Self::Input) -> Result<Self::Output, Error> {
        let shared = input.shared.clone();
        let data = input.into_inner();

        if data.entities.is_empty() {
            return Ok(ParallelContext::new(Vec::new(), shared));
        }

        let image_bytes = match data.image_spans.first() {
            Some(span) => span.data.encode_png()?,
            None => return Ok(ParallelContext::new(data.entities, shared)),
        };

        let entities = self
            .agent
            .verify_entities(&image_bytes, data.entities)
            .await
            .map_err(|e| Error::runtime(e.to_string(), "ocr-verification", e.is_retryable()))?;

        Ok(ParallelContext::new(entities, shared))
    }
}
