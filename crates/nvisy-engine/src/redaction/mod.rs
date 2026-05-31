//! Redaction: policy evaluation + multimodal application.
//!
//! Phase 4 of the pipeline. Two steps:
//!
//! 1. **Evaluate**: match entities against policy rules to produce
//!    [`AuditEntry`]s.
//! 2. **Apply**: build per-modality codec instructions (text, image,
//!    audio) from decisions and apply them to the document, writing
//!    replacement values into audit records.
//!
//! Unlike extraction and detection, redaction has no expensive
//! per-run construction — there's no model to load or HTTP client
//! to set up. The [`RedactionConfig`] config supplies deployment-wide
//! fallback values for plan [`Redaction`] fields that aren't
//! explicitly set.
//!
//! [`AuditEntry`]: nvisy_ontology::provenance::AuditEntry

mod apply;
mod config;
mod evaluate;
mod plan;
mod strategy;

use std::marker::PhantomData;

use nvisy_core::Result;
use nvisy_ontology::entity::is_excluded;
use nvisy_ontology::modality::{Modality, Overlap};

pub use self::config::RedactionConfig;
pub use self::evaluate::ApplyRedactions;
use self::evaluate::TARGET;
pub use self::plan::Redaction;
use crate::envelope::DocumentEnvelope;
use crate::envelope::value_at::ValueAt;
use crate::pipeline::{ModalityKind, Phase, PhaseContext, PhaseInfo};

/// Redaction phase: evaluate policies, attach an [`AuditEntry<M>`]
/// to each [`EntityRecord<M>`] the policy chain decides on, then
/// hand off to the codec applicator.
///
/// Stateless beyond the modality marker — the shared
/// [`RedactionConfig`] is read from `ctx.run` each call, and the
/// per-call plan slice comes from `ctx.plan.redaction`. Mutates the
/// envelope's audit (decision + execution per entry) and the
/// underlying codec handle (via [`ApplyRedactions`]).
///
/// [`AuditEntry<M>`]: nvisy_ontology::provenance::AuditEntry
/// [`EntityRecord<M>`]: nvisy_ontology::provenance::EntityRecord
pub struct RedactionPhase<M: Modality> {
    _marker: PhantomData<fn() -> M>,
}

impl<M: Modality> RedactionPhase<M> {
    /// Build the phase. Stateless beyond the modality marker.
    pub fn new() -> Self {
        Self {
            _marker: PhantomData,
        }
    }
}

impl<M: Modality> Default for RedactionPhase<M> {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait::async_trait]
impl<M> Phase<M> for RedactionPhase<M>
where
    M: Modality + Overlap,
    DocumentEnvelope<M>: ValueAt<M> + ApplyRedactions,
{
    fn inspect(&self) -> PhaseInfo {
        PhaseInfo {
            name: "redaction",
            modality: ModalityKind::of::<M>(),
            mutating: true,
        }
    }

    async fn run(
        &self,
        ctx: &PhaseContext<'_, M>,
        envelope: &mut DocumentEnvelope<M>,
    ) -> Result<()> {
        let cfg = &ctx.plan.redaction;
        let section = &ctx.run.redaction_config;
        let default_threshold = cfg
            .confidence_threshold
            .unwrap_or(section.confidence_threshold);
        // `process_metadata` is plumbed into the phase for future use
        // by the metadata-stripping pass; resolved here from the same
        // precedence chain (plan override → deployment default).
        let _process_metadata = cfg.process_metadata.unwrap_or(section.process_metadata);

        run_redaction(default_threshold, envelope).await
    }
}

/// Body of the redaction phase, parameterised on the resolved
/// `default_threshold`. Split out as a free function so the
/// test-only path can drive it without going through a `PhaseContext`.
pub(crate) async fn run_redaction<M>(
    default_threshold: nvisy_ontology::primitive::ConfidenceThreshold,
    envelope: &mut DocumentEnvelope<M>,
) -> Result<()>
where
    M: Modality + Overlap,
    DocumentEnvelope<M>: ValueAt<M> + ApplyRedactions,
{
    if envelope.audit.records.is_empty() {
        return Ok(());
    }

    // Drop entities that overlap an Assert-strength Exclusion
    // annotation. Defence in depth: catches both well-meaning
    // detectors and LLMs that ignored the exclusion-hint prompt.
    let before_filter = envelope.audit.records.len();
    let annotations = std::mem::take(&mut envelope.document.annotations);
    envelope
        .audit
        .records
        .retain(|record| !is_excluded(&annotations, &record.entity));
    envelope.document.annotations = annotations;
    let dropped = before_filter - envelope.audit.records.len();
    if dropped > 0 {
        tracing::debug!(
            target: TARGET,
            dropped,
            "filtered entities by Assert exclusion annotations",
        );
    }

    if envelope.audit.records.is_empty() {
        return Ok(());
    }

    let metadata = envelope.metadata.clone();
    let document_labels: Vec<&str> = envelope
        .document
        .labels
        .iter()
        .map(|l| l.label.as_str())
        .collect();

    let mut records = std::mem::take(&mut envelope.audit.records);
    evaluate::evaluate::<M>(
        &mut records,
        default_threshold,
        &document_labels,
        &metadata,
        envelope,
    )
    .await;
    envelope.audit.records = records;

    tracing::debug!(
        target: TARGET,
        entries = envelope.audit.entries().count(),
        "policy evaluation complete",
    );

    envelope.apply_pending().await?;
    Ok(())
}
