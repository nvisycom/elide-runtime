//! [`LlmRecognizer`]: LLM-driven NER over `nvisy_provider::NerAgent`.
//!
//! Wraps a constructed `NerAgent` with a frozen `DetectionConfig`
//! (entity-kind allowlist, score threshold, system prompt). The
//! agent manages coreference state internally between successive
//! `recognize` calls; call [`reset`](Self::reset) at document
//! boundaries to clear it.
//!
//! Construction is fallible — building the underlying agent
//! requires a configured LLM provider. The orchestrator treats
//! "no provider" as "skip this recognizer," not a fatal error.

use async_trait::async_trait;
use nvisy_ontology::entity::Entities;
use nvisy_rig::agent::{DetectionConfig, NerAgent};

use crate::error::{Error, Result};
use crate::{DetectionContext, Recognizer};

/// LLM-backed entity recognizer.
pub struct LlmRecognizer {
    agent: NerAgent,
    config: DetectionConfig,
}

impl LlmRecognizer {
    /// Construct from a pre-built [`NerAgent`] and a frozen
    /// detection config (entity-kind allowlist, score threshold,
    /// system prompt).
    pub fn new(agent: NerAgent, config: DetectionConfig) -> Self {
        Self { agent, config }
    }
}

#[async_trait]
impl Recognizer for LlmRecognizer {
    fn name(&self) -> &str {
        "llm"
    }

    async fn recognize(&self, ctx: &DetectionContext<'_>) -> Result<Entities> {
        let entities: Entities = self
            .agent
            .detect_entities(ctx.text, &self.config)
            .await
            .map_err(|e| Error::Recognizer {
                name: "llm".into(),
                cause: e.to_string(),
            })?
            .into();
        Ok(entities)
    }

    /// Clears coreference state on the wrapped `NerAgent`.
    async fn reset(&self) {
        self.agent.reset().await;
    }
}
