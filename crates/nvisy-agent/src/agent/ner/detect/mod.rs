//! Named Entity Recognition (NER) agent for textual PII/entity detection.
//!
//! [`NerAgent`] wraps a [`BaseAgent`] with NER-specific prompts. It
//! is a pure LLM agent (no tools) that analyses text, resolves the
//! LLM's candidates back into source byte ranges via the shared
//! offset resolver, and emits ready-to-use [`Entity<Text>`] values
//! whose trail starts with a recognition step carrying either a
//! model provenance (fresh discoveries) or an annotation provenance
//! (responses to per-call hints).
//!
//! Implements [`EntityRecognizer<Text>`] so it composes with the
//! rest of the platform through the same trait every other text
//! recognizer uses. Per-document hints + labels flow in via
//! [`RecognizerInput`].
//!
//! [`BaseAgent`]: super::BaseAgent
//! [`Entity<Text>`]: nvisy_core::entity::Entity
//! [`EntityRecognizer<Text>`]: nvisy_core::EntityRecognizer
//! [`RecognizerInput`]: nvisy_core::RecognizerInput

mod build;
mod localize;
mod output;
mod prompt;

use async_trait::async_trait;
use nvisy_core::entity::{AnnotationProvenance, ModelProvenance, TrailProvenance, TrailStep};
use nvisy_core::modality::Text;
use nvisy_core::{EntityRecognizer, RecognizerInput, RecognizerOutput, Result};
use uuid::Uuid;

use self::build::build_entities;
pub use self::localize::UnresolvedCandidatePolicy;
use self::localize::localize_all;
pub use self::output::NerCandidate;
use self::output::NerCandidates;
use self::prompt::{NER_SYSTEM_PROMPT, NerPromptBuilder};
use crate::agent::base::{BaseAgent, UsageTracker};
use crate::agent::{AgentConfig, AgentProvider};

const TARGET: &str = "nvisy_agent::agent::ner";

/// Agent for textual PII/entity detection using LLM-based NER.
///
/// # Workflow
///
/// 1. Caller calls [`recognize`] with a [`RecognizerInput<Text>`]
///    carrying the source text plus any uploader-supplied hints and
///    document labels.
/// 2. The agent builds a user prompt via `NerPromptBuilder` that
///    folds in any hints for per-hint adjudication alongside
///    open-ended discovery.
/// 3. Structured output is parsed into `Vec<NerCandidate>`,
///    localized back into byte ranges via the shared localizer,
///    and lifted into a [`RecognizerOutput<Text>`] stamped with
///    this agent's model provenance (or annotation provenance for
///    candidates carrying `hint_id`).
///
/// Stateless — no cross-call coreference tracking.
///
/// [`recognize`]: EntityRecognizer::recognize
pub struct NerAgent {
    base: BaseAgent,
    unresolved: UnresolvedCandidatePolicy,
}

impl NerAgent {
    /// Create a new NER agent. The HTTP client is built internally
    /// from `config.max_retries` and otherwise-default settings.
    /// `config.preamble` falls back to the built-in NER system
    /// prompt when unset.
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
    #[must_use]
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
}

#[async_trait]
impl EntityRecognizer<Text> for NerAgent {
    /// Detect entities in `input.data.text`.
    ///
    /// Performs unified entity detection: open-ended discovery
    /// across the source text **plus** per-hint adjudication for
    /// any [`hints`] on the input. Returns ready-to-use
    /// [`Entity<Text>`] values whose trail starts with a
    /// recognition step:
    ///
    /// - Candidates carrying `hint_id = Some(i)` get an
    ///   [`Annotation`] provenance
    ///   using `hints[i].name` (and the entity_id, if any, is
    ///   forwarded). Out-of-range `hint_id`s are treated as fresh
    ///   discoveries.
    /// - Candidates without `hint_id` get a
    ///   [`Model`] provenance carrying
    ///   this agent's model name.
    ///
    /// Candidates the localizer can't resolve are dropped per
    /// this agent's [`unresolved_policy`].
    ///
    /// [`Entity<Text>`]: nvisy_core::entity::Entity
    /// [`hints`]: nvisy_core::RecognizerInput::hints
    /// [`unresolved_policy`]: NerAgent::with_unresolved_policy
    /// [`Annotation`]: TrailProvenance::Annotation
    /// [`Model`]: TrailProvenance::Model
    #[tracing::instrument(
        target = TARGET,
        skip_all,
        fields(
            text_len = input.data.text.len(),
            hint_count = input.hints.len(),
            label_count = input.labels.len(),
        ),
    )]
    async fn recognize(&self, input: &RecognizerInput<Text>) -> Result<RecognizerOutput<Text>> {
        let text = input.data.text.as_str();
        let hints = &input.hints;
        let labels = &input.labels;

        let prompt = NerPromptBuilder::new(text, hints, labels).build();

        tracing::debug!(
            target: TARGET,
            prompt_len = prompt.len(),
            "built ner prompt"
        );

        let result: NerCandidates = self
            .base
            .prompt_structured(&prompt)
            .await
            .map_err(crate::error::convert)?;

        let candidate_count = result.entities.len();
        let localized = localize_all(text, result.entities, self.unresolved);
        let model_name = self.base.model_name().to_owned();
        let model = ModelProvenance::new(model_name.clone());
        let entities = build_entities(localized, |l, confidence| match l.candidate.hint_id {
            Some(i) if i < hints.len() => {
                let provenance = TrailProvenance::Annotation(AnnotationProvenance {
                    name: hints[i].name.clone(),
                });
                TrailStep::recognition("llm-ner", confidence, provenance, "")
            }
            _ => {
                let provenance = TrailProvenance::Model(model.clone());
                let reason = format!("llm '{model_name}' identified entity");
                TrailStep::recognition("llm-ner", confidence, provenance, reason)
            }
        });

        tracing::info!(
            target: TARGET,
            candidate_count,
            entity_count = entities.len(),
            "ner detection complete"
        );

        Ok(RecognizerOutput::new(entities))
    }

    async fn reset(&self) {
        self.base.tracker().reset();
    }
}
