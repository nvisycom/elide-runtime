//! Redaction operation.
//!
//! Evaluates policy rules against detected entities to produce
//! redaction records, then builds and applies redaction instructions
//! for the envelope's modality via [`RedactionApplicator`].

use nvisy_core::Result;
#[allow(unused_imports)] // for future per-modality apply blocks
use nvisy_ontology::modality::{Audio, Image, Tabular};
use nvisy_ontology::modality::{Modality, Text};

use super::apply::RedactionApplicator;
use super::defaults::RedactionDefaults;
use crate::envelope::DocumentEnvelope;
use crate::redaction::Redaction as RedactionConfig;

const TARGET: &str = "nvisy_engine::redaction";

/// Redaction operation: evaluates policies and applies redaction
/// instructions to a modality-typed envelope.
pub struct Redactor {
    default_threshold: f64,
    process_metadata: bool,
}

impl Redactor {
    /// Build from workflow config + server-wide defaults.
    pub fn new(cfg: &RedactionConfig, defaults: &RedactionDefaults) -> Self {
        Self {
            default_threshold: cfg
                .confidence_threshold
                .unwrap_or(defaults.confidence_threshold),
            process_metadata: cfg.process_metadata.unwrap_or(defaults.process_metadata),
        }
    }

    /// `true` when metadata stripping is enabled.
    #[must_use]
    pub fn process_metadata(&self) -> bool {
        self.process_metadata
    }

    /// Threshold below which entities are skipped for redaction.
    #[must_use]
    pub fn default_threshold(&self) -> f64 {
        self.default_threshold
    }

    /// Evaluate policies and apply redaction instructions to the
    /// envelope.
    ///
    /// Policy evaluation against the new `Strategy` enum + multi-
    /// modality envelopes is a follow-up; for now this is a no-op
    /// pass-through so the rest of the engine compiles end-to-end
    /// while the policy logic is redesigned.
    pub async fn execute<M: Modality>(&self, envelope: &mut DocumentEnvelope<M>) -> Result<()>
    where
        DocumentEnvelope<M>: ApplyRedactions,
    {
        if envelope.audit.entities.is_empty() {
            return Ok(());
        }

        tracing::warn!(
            target: TARGET,
            entities = envelope.audit.entities.len(),
            "policy evaluation against the typed envelope is not yet \
             reimplemented; skipping redaction pass",
        );

        // Wire-through: keep the applicator hook in place so the
        // per-modality redaction wiring is exercised once policy
        // evaluation is reinstated.
        let _ = envelope;
        Ok(())
    }
}

/// Per-modality applicator hook. Each modality envelope opts in via
/// a thin impl that calls the codec's typed redaction method; the
/// generic [`Redactor::execute`] above is parameterised over this so
/// the apply path is shared.
#[async_trait::async_trait]
pub trait ApplyRedactions {
    async fn apply_pending(&mut self) -> Result<()>;
}

#[async_trait::async_trait]
impl ApplyRedactions for DocumentEnvelope<Text> {
    async fn apply_pending(&mut self) -> Result<()> {
        RedactionApplicator::new(self).apply().await
    }
}
