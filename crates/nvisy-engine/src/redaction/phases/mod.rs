//! Redaction-side per-document phase orchestrators: the redaction
//! phase itself plus the post-redaction validation leak check.
//!
//! [`RedactionPhase`] drives a two-step flow per node:
//!
//! 1. **Evaluate** — for each detected entity, the [`PolicyStore`]
//!    picks `Redact` / `Suppress` / `Audit` / `Fallthrough`. The
//!    result lands on `EntityRecord::audit` as an
//!    [`AuditEntry<M>`] whose `decision` carries the resolved
//!    [`ResolvedAction<M>`] — for `Redact`, the per-modality
//!    operator picked from the rule's [`ModalityRedactions`] (or
//!    the deployment-wide default if the rule didn't cover the
//!    entity's modality).
//! 2. **Apply** — pending entities have their operator spec
//!    instantiated via [`Instantiate::instantiate`]; built-in
//!    arms construct a fresh operator from the rule's params,
//!    `Custom` arms look up the user-registered operator in the
//!    [`RedactionRegistry<M>`]. The phase pulls the per-entity
//!    source payload through [`DataAt`] and calls
//!    [`Anonymizer::apply`]; the produced `M::Replacement` is
//!    written back as `Execution::Applied { replacement }`.
//!    Operator construction errors and `apply` errors both fail
//!    the record into `Execution::Failed` with the underlying
//!    reason.
//!
//! [`ValidationPhase`] runs after redaction: it streams the
//! redacted output back through the codec, walks the audit's
//! applied entities, and runs a per-modality [`LeakCheck`] over
//! the redacted text to surface any value that should have been
//! redacted but still appears in the output.
//!
//! Codec-side byte mutation (actually writing the replacement
//! back at the entity's location in the handle) is still a
//! separate follow-up — recorded as a TODO inline.
//!
//! [`Anonymizer::apply`]: nvisy_toolkit::redaction::Anonymizer::apply
//! [`RedactionRegistry<M>`]: nvisy_toolkit::redaction::RedactionRegistry
//! [`DataAt`]: nvisy_core::extraction::DataAt
//! [`ResolvedAction<M>`]: crate::document::provenance::ResolvedAction
//! [`ModalityRedactions`]: crate::policy::redaction::ModalityRedactions
//! [`LeakCheck`]: nvisy_toolkit::validation::LeakCheck

mod instantiate;
pub mod phase;
mod registries;
pub mod validation;

use nvisy_codec::content::ContentDescriptor;
use nvisy_core::Result;
use nvisy_core::entity::is_excluded;
use nvisy_core::extraction::{DataAt, TextAt};
use nvisy_core::modality::Overlap;
use nvisy_core::primitive::ConfidenceThreshold;
use nvisy_core::redaction::{RedactAt, Redactions};
use nvisy_toolkit::redaction::RedactionRegistry;

pub use self::instantiate::Instantiate;
pub use self::phase::RedactionPhase;
pub use self::registries::RedactionRegistries;
pub use self::validation::ValidationPhase;
use crate::core::{Decision, DocumentTree, PolicyStore};
use crate::document::provenance::{
    AuditEntry, Decision as AuditDecision, EntryMetadata, Execution, ResolvedAction,
};
use crate::modality::DocumentModality;
use crate::policy::PolicyDecisionRef;
use crate::policy::redaction::{ModalityRedactions, ProjectRedaction};

pub(crate) const TARGET: &str = "nvisy_engine::redaction";

/// Body of the redaction phase, parameterised on the resolved
/// `default_threshold`. Crate-visible so both the phase
/// orchestrator and the test-only path drive it through the same
/// code.
pub(crate) async fn run_redaction<M>(
    default_threshold: ConfidenceThreshold,
    tree: &mut DocumentTree<M>,
    descriptor: &ContentDescriptor,
    policies: &PolicyStore,
    catalog: &nvisy_core::entity::EntityLabelCatalog,
    default_operators: &ModalityRedactions,
    registry: &RedactionRegistry<M>,
) -> Result<()>
where
    M: DocumentModality + ProjectRedaction,
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

    for record in &mut tree.root.audit.records {
        if record.audit.is_some() {
            continue;
        }
        if !default_threshold.admits(record.entity.confidence) {
            continue;
        }
        let decision = policies.resolve::<M>(
            &record.entity,
            catalog,
            default_operators,
            &document_labels,
            descriptor,
        );
        let entry = match decision {
            Decision::Redact {
                policy_name,
                rule_name,
                operator,
            } => AuditEntry {
                decision: AuditDecision {
                    policy_ref: Some(PolicyDecisionRef::new(policy_name, rule_name)),
                    action: ResolvedAction::Redact { operator },
                },
                execution: Execution::Pending,
                metadata: EntryMetadata::now(),
            },
            Decision::Suppress {
                policy_name,
                rule_name,
                reason,
            } => AuditEntry {
                decision: AuditDecision {
                    policy_ref: Some(PolicyDecisionRef::new(policy_name, rule_name)),
                    action: ResolvedAction::Suppress { reason },
                },
                execution: Execution::Suppressed,
                metadata: EntryMetadata::now(),
            },
            Decision::Audit {
                policy_name,
                rule_name,
                severity,
            } => AuditEntry {
                decision: AuditDecision {
                    policy_ref: Some(PolicyDecisionRef::new(policy_name, rule_name)),
                    action: ResolvedAction::Audit { severity },
                },
                execution: Execution::Suppressed,
                metadata: EntryMetadata::now(),
            },
            Decision::Fallthrough => continue,
        };
        record.audit = Some(entry);
    }

    let mut apply_outcomes: Vec<(usize, Execution<M>)> = Vec::new();

    for (idx, record) in tree.root.audit.records.iter().enumerate() {
        let Some(audit) = record.audit.as_ref() else {
            continue;
        };
        if !matches!(audit.execution, Execution::Pending) {
            continue;
        }
        let ResolvedAction::Redact { operator } = &audit.decision.action else {
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

    Ok(())
}
