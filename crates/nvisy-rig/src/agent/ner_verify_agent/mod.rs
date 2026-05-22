//! [`NerVerifyAgent`]: localize + (optionally) refine LLM-produced NER
//! candidates into [`Entity`] values.
//!
//! Two responsibilities:
//!
//! 1. **Localize.** Resolve each [`NerCandidate`]'s surface form
//!    into a byte range in the source text by searching for
//!    `candidate.context`. Candidates whose context is absent or
//!    ambiguous (zero or many matches) are dropped per
//!    [`UnresolvedCandidatePolicy`].
//!
//! 2. **Refine (optional).** When constructed with
//!    [`with_refinement`], prompts the LLM a second time with the
//!    localized candidates plus the source text and asks it to
//!    correct/reject entries that don't survive a closer look.
//!    Mirrors the [`CvVerifyAgent`] verification flow; same
//!    [`VerificationOutput`] shape.
//!
//! Output is `Vec<Entity>` ready for the deduplication phase.
//! Recognition method is set to [`RecognitionMethod::Ner`] with
//! the agent's model provenance; refinement adds
//! [`RefinementMethod::ModelVerification`].
//!
//! [`with_refinement`]: NerVerifyAgent::with_refinement
//! [`CvVerifyAgent`]: crate::agent::CvVerifyAgent
//! [`RecognitionMethod::Ner`]: nvisy_ontology::entity::RecognitionMethod::Ner
//! [`RefinementMethod::ModelVerification`]: nvisy_ontology::entity::RefinementMethod::ModelVerification

mod localize;
mod prompt;
mod refine;

use nvisy_core::Result;
use nvisy_ontology::entity::{
    Entities, Entity, EntityCategory, EntityKind, Location, ModelKind, ModelProvenance,
    RecognitionMethod, RefinementMethod, TextLocation,
};
use nvisy_ontology::primitive::Confidence;

pub use self::localize::UnresolvedCandidatePolicy;
use self::localize::{LocalizedCandidate, localize_all};
use self::prompt::NER_VERIFIER_SYSTEM_PROMPT;
use self::refine::refine_localized;
use crate::agent::base::UsageTracker;
use crate::agent::ner::NerCandidate;
use crate::agent::{AgentConfig, AgentProvider, BaseAgent};

const TARGET: &str = "nvisy_rig::agent::ner_verify_agent";

/// Default confidence assigned to a candidate when the LLM didn't
/// score it.
const DEFAULT_CONFIDENCE: f64 = 0.5;

/// Verifier for NER candidates.
#[derive(Default)]
pub struct NerVerifyAgent {
    /// LLM agent for the optional refinement pass. `None` means
    /// pure-localization verification (no second LLM call).
    refiner: Option<BaseAgent>,
    /// What to do with candidates that can't be uniquely localized.
    unresolved: UnresolvedCandidatePolicy,
}

impl NerVerifyAgent {
    /// Localization-only verifier. No second LLM call.
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a second-pass LLM refiner. The refiner gets the source
    /// text plus the localized candidates and may
    /// confirm/correct/reject each.
    pub fn with_refinement(
        mut self,
        provider: &AgentProvider,
        mut config: AgentConfig,
    ) -> Result<Self> {
        config
            .preamble
            .get_or_insert_with(|| NER_VERIFIER_SYSTEM_PROMPT.into());
        let agent = BaseAgent::builder(provider, config)
            .build()
            .map_err(crate::error::convert)?;
        self.refiner = Some(agent);
        Ok(self)
    }

    /// Configure how unresolvable candidates are handled.
    pub fn with_unresolved_policy(mut self, policy: UnresolvedCandidatePolicy) -> Self {
        self.unresolved = policy;
        self
    }

    /// Access the usage tracker for the optional refiner agent.
    pub fn tracker(&self) -> Option<&UsageTracker> {
        self.refiner.as_ref().map(|a| a.tracker())
    }

    /// UUID of the refiner agent, or `None` when no refiner is
    /// configured.
    pub fn id(&self) -> Option<uuid::Uuid> {
        self.refiner.as_ref().map(|a| a.id())
    }

    /// Refiner agent's model name, or `None` when no refiner is
    /// configured.
    pub fn model_name(&self) -> Option<&str> {
        self.refiner.as_ref().map(|a| a.model_name())
    }

    /// Verify candidates against the source text.
    #[tracing::instrument(
        target = TARGET,
        skip_all,
        fields(text_len = text.len(), candidate_count = candidates.len()),
    )]
    pub async fn verify(&self, text: &str, candidates: Vec<NerCandidate>) -> Result<Entities> {
        // 1. Localize.
        let mut localized = localize_all(text, candidates, self.unresolved);
        let refined = self.refiner.is_some();

        // 2. Optional refinement pass.
        if let Some(ref refiner) = self.refiner {
            localized = refine_localized(refiner, text, localized).await?;
        }

        // 3. Build Entities.
        Ok(self.build_entities(localized, refined))
    }

    fn build_entities(&self, localized: Vec<LocalizedCandidate>, refined: bool) -> Entities {
        let model_name = self
            .refiner
            .as_ref()
            .map(|a| a.model_name().to_string())
            .unwrap_or_else(|| "ner_verify_agent".to_string());
        let model = ModelProvenance::new(&model_name, ModelKind::Gateway);

        let mut out = Entities::new();
        let mut dropped_missing_kind = 0usize;
        let mut dropped_bad_confidence = 0usize;
        for l in localized {
            let entity_kind: EntityKind = match l.candidate.entity_type {
                Some(k) => k,
                None => {
                    dropped_missing_kind += 1;
                    continue;
                }
            };
            // Trust the LLM's category if it gave one; otherwise
            // derive it from the entity_kind via the ontology
            // mapping (PersonalIdentity / Financial / Health /
            // ContactInfo / etc.). Guarantees consistency between
            // kind and category, which a fixed-default would not.
            let category: EntityCategory = l
                .candidate
                .category
                .unwrap_or_else(|| entity_kind.category());
            let raw = l.candidate.confidence.unwrap_or(DEFAULT_CONFIDENCE);
            let confidence = match Confidence::new(raw.clamp(0.0, 1.0)) {
                Some(c) => c,
                None => {
                    dropped_bad_confidence += 1;
                    continue;
                }
            };

            let loc = TextLocation::builder()
                .with_start_offset(l.start_offset)
                .with_end_offset(l.end_offset)
                .build()
                .expect("required fields provided");

            let mut refinement_methods = Vec::new();
            if refined {
                refinement_methods.push(RefinementMethod::ModelVerification);
            }

            let mut b = Entity::builder()
                .with_category(category)
                .with_entity_kind(entity_kind)
                .with_recognition_methods(vec![RecognitionMethod::Ner(model.clone())])
                .with_refinement_methods(refinement_methods)
                .with_confidence(confidence)
                .with_location(Location::from(loc));
            if let Some(id) = l.candidate.entity_id {
                b = b.with_entity_id(id);
            }
            out.push(b.build().expect("required fields provided"));
        }

        if dropped_missing_kind > 0 || dropped_bad_confidence > 0 {
            tracing::debug!(
                target: TARGET,
                dropped_missing_kind,
                dropped_bad_confidence,
                "dropped candidates during entity construction"
            );
        }
        out
    }
}
