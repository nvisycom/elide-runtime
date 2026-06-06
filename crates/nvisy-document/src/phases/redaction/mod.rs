//! Redaction phase: walks per-modality entities, asks the
//! per-modality [`PolicyStore`] for the action, instantiates the
//! rule's operator spec, runs it against the source payload, and
//! lands the produced replacement on the audit record as
//! [`Execution::Applied`].
//!
//! Two-step flow per node:
//!
//! 1. **Evaluate** — for each detected entity, the per-modality
//!    [`PolicyStore`] picks `Redact` / `Suppress` / `Fallthrough`.
//!    The result lands on `EntityRecord::audit` as an
//!    [`AuditEntry<M>`] whose `decision` carries the winning rule's
//!    [`Action<M>`] verbatim (operator spec included for `Redact`
//!    arms).
//! 2. **Apply** — pending entities have their operator spec
//!    instantiated via [`Instantiate::instantiate`]; built-in arms
//!    construct a fresh operator from the rule's params, `Custom`
//!    arms look up the user-registered operator in the
//!    [`RedactionRegistry<M>`]. The phase pulls the per-entity
//!    source payload through [`DataAt`] and calls
//!    [`Anonymizer::apply`]; the produced `M::Replacement` is
//!    written back as [`Execution::Applied { replacement }`].
//!    Operator construction errors and `apply` errors both fail the
//!    record into [`Execution::Failed`] with the underlying reason.
//!
//! Codec-side byte mutation (actually writing the replacement back
//! at the entity's location in the handle) is still a separate
//! follow-up — recorded as a TODO inline.
//!
//! [`Anonymizer::apply`]: nvisy_toolkit::redaction::Anonymizer::apply
//! [`RedactionRegistry<M>`]: nvisy_toolkit::redaction::RedactionRegistry
//! [`DataAt`]: nvisy_core::extraction::DataAt

pub mod phase;
mod registries;

use nvisy_core::content::ContentMetadata;
use nvisy_core::entity::is_excluded;
use nvisy_core::extraction::DataAt;
use nvisy_core::modality::Overlap;
use nvisy_core::primitive::ConfidenceThreshold;
use nvisy_core::redaction::{RedactAt, Redactions};
use nvisy_core::{Result, TextAt};
use nvisy_toolkit::redaction::RedactionRegistry;

pub use self::registries::RedactionRegistries;
use crate::core::{Decision, DocumentTree, PolicyStore};
use crate::modality::DocumentModality;
use crate::policy::Action;
use crate::policy::redaction::Instantiate;
use crate::provenance::{AuditEntry, Decision as AuditDecision, EntryMetadata, Execution};

pub(crate) const TARGET: &str = "nvisy_engine::redaction";

/// Body of the redaction phase, parameterised on the resolved
/// `default_threshold`. Crate-visible so both the phase orchestrator
/// and the test-only path drive it through the same code.
pub(crate) async fn run_redaction<M>(
    default_threshold: ConfidenceThreshold,
    tree: &mut DocumentTree<M>,
    metadata: &ContentMetadata,
    policies: &PolicyStore,
    registry: &RedactionRegistry<M>,
) -> Result<()>
where
    M: DocumentModality,
    M::Location: Overlap,
    M::Redaction: Instantiate<M>,
    DocumentTree<M>: TextAt<M> + DataAt<M> + RedactAt<M>,
{
    if tree.root.audit.records.is_empty() {
        return Ok(());
    }

    let before_filter = tree.root.audit.records.len();
    let annotations = std::mem::take(&mut tree.root.annotations);
    tree.root
        .audit
        .records
        .retain(|record| !is_excluded(&annotations, &record.entity));
    tree.root.annotations = annotations;
    let dropped = before_filter - tree.root.audit.records.len();
    if dropped > 0 {
        tracing::debug!(
            target: TARGET,
            dropped,
            "filtered entities by Assert exclusion annotations",
        );
    }

    if tree.root.audit.records.is_empty() {
        return Ok(());
    }

    let document_labels: Vec<&str> = tree.root.labels.iter().map(|l| l.label.as_str()).collect();

    // Resolve policies first; defer the apply step so we can borrow
    // the records mutably without holding the immutable doc fields.
    for record in &mut tree.root.audit.records {
        if record.audit.is_some() {
            continue;
        }
        if !default_threshold.admits(record.entity.confidence) {
            continue;
        }
        let decision = policies.resolve::<M>(&record.entity, &document_labels, metadata);
        let entry = match decision {
            Decision::Redact {
                policy_id,
                rank,
                operator,
            } => AuditEntry {
                decision: AuditDecision {
                    policy_id: Some(policy_id),
                    rank: Some(rank),
                    action: Action::Redact { operator },
                },
                execution: Execution::Pending,
                metadata: EntryMetadata::now(),
            },
            Decision::Suppress { policy_id, rank } => AuditEntry {
                decision: AuditDecision {
                    policy_id: Some(policy_id),
                    rank: Some(rank),
                    action: Action::Suppress,
                },
                execution: Execution::Suppressed,
                metadata: EntryMetadata::now(),
            },
            Decision::Fallthrough => continue,
        };
        record.audit = Some(entry);
    }

    // Apply step: walk the records that landed `Pending`, instantiate
    // the rule's operator spec, pull the per-entity source payload
    // through `DataAt` on the tree, run the operator, write the
    // produced replacement back as `Execution::Applied`. Operator
    // construction errors and `apply` errors flip the record to
    // `Execution::Failed`.
    let mut apply_outcomes: Vec<(usize, Execution<M>)> = Vec::new();

    for (idx, record) in tree.root.audit.records.iter().enumerate() {
        let Some(audit) = record.audit.as_ref() else {
            continue;
        };
        if !matches!(audit.execution, Execution::Pending) {
            continue;
        }
        let Action::Redact { operator } = &audit.decision.action else {
            continue;
        };
        let outcome = match operator.instantiate(registry) {
            Err(err) => Execution::Failed {
                reason: err.to_string(),
            },
            Ok(anonymizer) => match tree.data_at(&record.entity.location).await {
                None => Execution::Failed {
                    reason: "source payload unavailable at entity location".into(),
                },
                Some(source) => match anonymizer.apply(&record.entity, &source).await {
                    Ok(replacement) => Execution::Applied { replacement },
                    Err(err) => Execution::Failed {
                        reason: err.to_string(),
                    },
                },
            },
        };
        apply_outcomes.push((idx, outcome));
    }

    // Collect the produced replacements into a Redactions batch and
    // flush through `RedactAt` so the codec handle rewrites the
    // underlying bytes. The batch mirrors the audit; if write-back
    // fails, the audit still reflects what the operators produced.
    let mut redactions = Redactions::<M>::new();
    for (idx, outcome) in apply_outcomes {
        if let Execution::Applied { replacement } = &outcome {
            redactions.push(
                tree.root.audit.records[idx].entity.location.clone(),
                replacement.clone(),
            );
        }
        if let Some(audit) = tree.root.audit.records[idx].audit.as_mut() {
            audit.execution = outcome;
        }
    }

    if !redactions.is_empty() {
        let count = redactions.len();
        tree.redact_at(redactions).await?;
        tracing::debug!(
            target: TARGET,
            count,
            "flushed redaction batch through codec handle",
        );
    }

    tracing::debug!(
        target: TARGET,
        entries = tree.root.audit.entries().count(),
        "policy evaluation + operator dispatch complete",
    );

    Ok(())
}
