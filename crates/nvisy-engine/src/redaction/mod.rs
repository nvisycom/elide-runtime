//! Redaction: policy evaluation + multimodal application.
//!
//! Two steps run by `run_redaction`:
//!
//! 1. **Evaluate**: match entities against policy rules to produce
//!    [`AuditEntry`]s.
//! 2. **Apply**: build per-modality codec instructions (text, image,
//!    audio) from decisions and apply them to the document, writing
//!    replacement values into audit records.
//!
//! Unlike extraction and detection, redaction has no expensive
//! per-run construction — there's no model to load or HTTP client
//! to set up. The phase orchestrator
//! ([`RedactionPhase`](crate::pipeline::RedactionPhase)) holds a
//! [`RedactionConfig`](crate::pipeline::RedactionConfig) that
//! supplies deployment-wide fallback values for plan
//! [`Redaction`](crate::pipeline::Redaction) fields that aren't
//! explicitly set.
//!
//! [`AuditEntry`]: nvisy_ontology::provenance::AuditEntry

mod apply;
mod evaluate;
mod strategy;

use nvisy_core::Result;
use nvisy_core::content::ContentMetadata;
use nvisy_ontology::document::Document;
use nvisy_ontology::entity::is_excluded;
use nvisy_ontology::modality::{Modality, Overlap};
use nvisy_ontology::primitive::ConfidenceThreshold;

use self::evaluate::TARGET;
pub use self::evaluate::{ApplyRedactions, ApplyRedactionsImpl};
use crate::core::{DocumentView, PolicyStore, SharedHandle, ValueAt};

/// Body of the redaction phase, parameterised on the resolved
/// `default_threshold`. Public to the crate so both the phase
/// orchestrator and the test-only path drive it through the same
/// code.
pub(crate) async fn run_redaction<M>(
    default_threshold: ConfidenceThreshold,
    doc: &mut Document<M>,
    handle: &SharedHandle,
    metadata: &ContentMetadata,
    policies: &PolicyStore,
) -> Result<()>
where
    M: Modality + Overlap,
    for<'a> DocumentView<'a, M>: ValueAt<M>,
    ApplyRedactionsImpl: ApplyRedactions<M>,
{
    if doc.audit.records.is_empty() {
        return Ok(());
    }

    // Drop entities that overlap an Assert-strength Exclusion
    // annotation. Defence in depth: catches both well-meaning
    // detectors and LLMs that ignored the exclusion-hint prompt.
    let before_filter = doc.audit.records.len();
    let annotations = std::mem::take(&mut doc.annotations);
    doc.audit
        .records
        .retain(|record| !is_excluded(&annotations, &record.entity));
    doc.annotations = annotations;
    let dropped = before_filter - doc.audit.records.len();
    if dropped > 0 {
        tracing::debug!(
            target: TARGET,
            dropped,
            "filtered entities by Assert exclusion annotations",
        );
    }

    if doc.audit.records.is_empty() {
        return Ok(());
    }

    let document_labels: Vec<&str> = doc.labels.iter().map(|l| l.label.as_str()).collect();

    let mut records = std::mem::take(&mut doc.audit.records);
    evaluate::evaluate::<M>(
        &mut records,
        default_threshold,
        &document_labels,
        metadata,
        policies,
    )
    .await;
    doc.audit.records = records;

    tracing::debug!(
        target: TARGET,
        entries = doc.audit.entries().count(),
        "policy evaluation complete",
    );

    <ApplyRedactionsImpl as ApplyRedactions<M>>::apply_pending(doc, handle).await?;
    Ok(())
}
