//! Unified OCR agent: extraction + optional LLM-based verification.
//!
//! [`OcrAgent`] wraps an [`OcrEngine`](crate::ocr::OcrEngine) with an
//! optional LLM verifier. It is the single public entry point for all
//! OCR operations.

mod input;
mod output;
mod prompt;

use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use nvisy_core::Error;
use nvisy_ontology::entity::Entity;
use uuid::Uuid;

pub use self::input::ProposedEntity;
pub use self::output::{VerificationOutput, VerificationStatus, VerifiedEntity};
use self::prompt::{OCR_SYSTEM_PROMPT, OcrPromptBuilder};
use crate::agent::base::UsageTracker;
use crate::agent::{AgentConfig, AgentProvider, BaseAgent};
use crate::http::HttpClient;
pub use crate::ocr::{Backend, Block, BlockKind, ImageFormat, Line, OcrProvider, Page, Word};
use crate::ocr::{ImageInput, ImageOutput, OcrEngine, RunParams};

const TARGET: &str = "nvisy_provider::agent::ocr";

/// Unified OCR agent: extraction via backend providers + optional
/// LLM-based verification of detected entities.
pub struct OcrAgent {
    engine: OcrEngine,
    params: RunParams,
    verifier: Option<BaseAgent>,
}

impl OcrAgent {
    /// Create an OCR agent with extraction only (no verification).
    pub fn new(ocr_provider: OcrProvider, params: RunParams, http_client: &HttpClient) -> Self {
        let engine = ocr_provider.into_engine_with_client(http_client.clone());
        Self {
            engine,
            params,
            verifier: None,
        }
    }

    /// Add LLM-based verification to this agent.
    pub fn with_verification(
        mut self,
        llm_provider: &AgentProvider,
        llm_config: AgentConfig,
    ) -> Result<Self, crate::error::Error> {
        let mut config = llm_config;
        config
            .preamble
            .get_or_insert_with(|| OCR_SYSTEM_PROMPT.into());
        self.verifier = Some(BaseAgent::builder(llm_provider, config).build()?);
        Ok(self)
    }

    /// Returns `true` if LLM verification is available.
    pub fn has_verifier(&self) -> bool {
        self.verifier.is_some()
    }

    /// Run OCR on a single image.
    #[tracing::instrument(
        target = TARGET,
        skip_all,
        fields(source = %image.source, image_bytes = image.len()),
    )]
    pub async fn run(&self, image: &ImageInput) -> Result<ImageOutput, Error> {
        self.engine.run(image, &self.params).await
    }

    /// Run OCR on multiple images.
    #[tracing::instrument(target = TARGET, skip_all, fields(count = images.len()))]
    pub async fn run_batch(&self, images: &[ImageInput]) -> Result<Vec<ImageOutput>, Error> {
        self.engine.run_batch(images, &self.params).await
    }

    /// Verify proposed entities against the original image using the LLM.
    ///
    /// Returns only entities that were corrected or rejected. Entities
    /// absent from the output are implicitly confirmed.
    ///
    /// Returns `Err` if no verifier is configured.
    #[tracing::instrument(
        target = TARGET,
        skip_all,
        fields(image_bytes = image.len(), entity_count = entities.len()),
    )]
    pub async fn verify(
        &self,
        image: &ImageInput,
        entities: &[ProposedEntity],
    ) -> Result<VerificationOutput, crate::error::Error> {
        let verifier = self.verifier.as_ref().ok_or_else(|| {
            crate::error::Error::Provider("OCR verification requires an LLM verifier".into())
        })?;

        let image_b64 = STANDARD.encode(&image.data);
        tracing::debug!(
            target: TARGET,
            b64_len = image_b64.len(),
            proposed = entities.len(),
            "encoded image, building verification prompt"
        );

        let prompt = OcrPromptBuilder::new(entities).build(&image_b64);
        let output: VerificationOutput = verifier.prompt_structured(&prompt).await?;

        tracing::info!(
            target: TARGET,
            changed = output.entities.len(),
            "ocr verification complete"
        );

        Ok(output)
    }

    /// Verify entities against the original image, returning the merged result.
    ///
    /// Confirmed entities pass through unchanged, corrected entities are
    /// updated, and rejected entities are dropped.
    ///
    /// Returns `Err` if no verifier is configured.
    #[tracing::instrument(
        target = TARGET,
        skip_all,
        fields(image_bytes = image.len(), entity_count = entities.len()),
    )]
    pub async fn verify_entities(
        &self,
        image: &ImageInput,
        entities: Vec<Entity>,
    ) -> Result<Vec<Entity>, crate::error::Error> {
        if entities.is_empty() {
            return Ok(Vec::new());
        }

        let proposed: Vec<ProposedEntity> = entities
            .iter()
            .enumerate()
            .map(|(i, e)| ProposedEntity::from_entity(i, e))
            .collect();

        let output = self.verify(image, &proposed).await?;
        Ok(output.merge(entities))
    }

    /// Access the usage tracker for the LLM verifier, if configured.
    pub fn tracker(&self) -> Option<&UsageTracker> {
        self.verifier.as_ref().map(|v| v.tracker())
    }

    /// Unique identifier for the verifier agent instance.
    pub fn verifier_id(&self) -> Option<Uuid> {
        self.verifier.as_ref().map(|v| v.id())
    }
}
