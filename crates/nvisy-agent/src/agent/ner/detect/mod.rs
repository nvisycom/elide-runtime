//! Named Entity Recognition (NER) agent for textual PII/entity detection.
//!
//! [`NerAgent`] wraps a [`BaseAgent`] with NER-specific prompts. It
//! is a pure LLM agent (no tools) that analyses text, resolves the
//! LLM's candidates back into source byte ranges via the shared
//! offset resolver, and emits ready-to-use [`Entity<Text>`] values
//! stamped with [`RecognitionMethod::LlmNer`] (fresh discoveries) or
//! [`RecognitionMethod::Annotation`] (responses to per-call hints).
//!
//! [`BaseAgent`]: super::BaseAgent
//! [`Entity<Text>`]: nvisy_ontology::entity::Entity
//! [`RecognitionMethod::LlmNer`]: nvisy_ontology::entity::RecognitionMethod::LlmNer
//! [`RecognitionMethod::Annotation`]: nvisy_ontology::entity::RecognitionMethod::Annotation

mod build;
mod localize;
mod output;
mod prompt;

use nvisy_core::Result;
use nvisy_ontology::entity::{Entity, ModelKind, ModelProvenance, RecognitionMethod};
use nvisy_ontology::modality::Text;
use uuid::Uuid;

use self::build::build_entities;
use self::localize::localize_all;
pub use self::localize::UnresolvedCandidatePolicy;
use self::output::NerCandidates;
pub use self::output::NerCandidate;
use self::prompt::{NER_SYSTEM_PROMPT, NerPromptBuilder};
use crate::agent::base::{BaseAgent, UsageTracker};
use crate::agent::{AgentConfig, AgentProvider, LlmNerContext};

const TARGET: &str = "nvisy_agent::agent::ner";

/// Agent for textual PII/entity detection using LLM-based NER.
///
/// # Workflow
///
/// 1. Caller passes the source text plus a [`LlmNerContext`] to
///    [`detect`].
/// 2. The agent builds a user prompt via `NerPromptBuilder` that
///    specifies entity types, confidence thresholds, and any
///    Hint-strength inclusion descriptions the uploader supplied.
/// 3. Structured output is parsed into `Vec<NerCandidate>`,
///    localized back into byte ranges via the shared localizer,
///    and lifted into `Vec<Entity<Text>>` stamped with this
///    agent's model provenance under
///    [`RecognitionMethod::LlmNer`].
///
/// Stateless — no cross-call coreference tracking.
///
/// [`detect`]: Self::detect
/// [`RecognitionMethod::LlmNer`]: nvisy_ontology::entity::RecognitionMethod::LlmNer
pub struct NerAgent {
    base: BaseAgent,
    unresolved: UnresolvedCandidatePolicy,
}

impl NerAgent {
    /// Create a new NER agent. The HTTP client is built internally
    /// from `config.max_retries` and otherwise-default settings.
    pub fn new(provider: &AgentProvider, mut config: AgentConfig) -> Result<Self> {
        config
            .preamble
            .get_or_insert_with(|| NER_SYSTEM_PROMPT.into());
        let base = BaseAgent::builder(provider, config)
            .build()
            .map_err(crate::error::convert)?;
        Ok(Self {
            base,
            unresolved: UnresolvedCandidatePolicy::default(),
        })
    }

    /// Configure how unresolvable candidates are handled.
    pub fn with_unresolved_policy(mut self, policy: UnresolvedCandidatePolicy) -> Self {
        self.unresolved = policy;
        self
    }

    /// Unique identifier for this agent instance (UUIDv7).
    pub fn id(&self) -> Uuid {
        self.base.id()
    }

    /// Access the usage tracker for this agent's LLM calls.
    pub fn tracker(&self) -> &UsageTracker {
        self.base.tracker()
    }

    /// The model name used by this agent.
    pub fn model_name(&self) -> &str {
        self.base.model_name()
    }

    /// Detect entities in text.
    ///
    /// Performs unified entity detection: open-ended discovery
    /// across the source text **plus** per-hint adjudication for
    /// any [`hints`] in `config`. Returns ready-to-use
    /// [`Entity<Text>`] values stamped with the appropriate
    /// recognition method:
    ///
    /// - Candidates carrying `hint_id = Some(i)` are stamped with
    ///   [`RecognitionMethod::Annotation`] using `hints[i].name`
    ///   (and the entity_id, if any, is forwarded). Out-of-range
    ///   `hint_id`s are treated as fresh discoveries.
    /// - Candidates without `hint_id` are stamped with
    ///   [`RecognitionMethod::LlmNer`] carrying this agent's
    ///   model provenance.
    ///
    /// Candidates the localizer can't resolve are dropped per
    /// this agent's [`unresolved_policy`].
    ///
    /// [`hints`]: crate::agent::LlmNerContext::hints
    /// [`unresolved_policy`]: Self::with_unresolved_policy
    /// [`RecognitionMethod::Annotation`]: nvisy_ontology::entity::RecognitionMethod::Annotation
    /// [`RecognitionMethod::LlmNer`]: nvisy_ontology::entity::RecognitionMethod::LlmNer
    #[tracing::instrument(
        target = TARGET,
        skip_all,
        fields(text_len = text.len(), hint_count = config.hints.len()),
    )]
    pub async fn detect(&self, text: &str, config: &LlmNerContext) -> Result<Vec<Entity<Text>>> {
        let prompt = NerPromptBuilder::new(config).build(text);

        tracing::debug!(
            target: TARGET,
            prompt_len = prompt.len(),
            entity_kinds = config.entity_kinds.len(),
            "built ner prompt"
        );

        let result: NerCandidates = self
            .base
            .prompt_structured(&prompt)
            .await
            .map_err(crate::error::convert)?;

        let candidate_count = result.entities.len();
        let localized = localize_all(text, result.entities, self.unresolved);
        let model = ModelProvenance::new(self.base.model_name(), ModelKind::Gateway);
        let llm_method = RecognitionMethod::LlmNer(model);
        let hints = &config.hints;
        let entities = build_entities(localized, |l| match l.candidate.hint_id {
            Some(i) if i < hints.len() => RecognitionMethod::annotation(hints[i].name.clone()),
            _ => llm_method.clone(),
        });

        tracing::info!(
            target: TARGET,
            candidate_count,
            entity_count = entities.len(),
            "ner detection complete"
        );

        Ok(entities)
    }
}
