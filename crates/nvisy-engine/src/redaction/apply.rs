//! Per-modality redaction applicator: materialises codec redaction
//! instructions from the [`EntityRecord`]s the evaluator decorated
//! with audit entries, and hands them off to the codec.
//!
//! One pass per envelope. The flow is the same for every modality:
//!
//! 1. Walk `document.audit.records`. For each record whose `audit`
//!    is present and [`Pending`], read the entity's `location` and
//!    `entity_kind` directly off the record — no lookup needed.
//! 2. Convert the entry's [`Strategy`] into a codec-side
//!    [`Codable::Redaction`] via the per-modality strategy
//!    converter.
//! 3. Insert the `(M, Redaction)` pair into a
//!    [`Redactions<M, R>`] collection.
//! 4. The per-modality caller hands the batch to the codec via the
//!    typed apply hook on the envelope, then commits per-entry
//!    audit state via [`commit`].
//!
//! [`EntityRecord`]: nvisy_ontology::provenance::EntityRecord
//! [`Pending`]: nvisy_ontology::provenance::Execution::Pending
//! [`Strategy`]: nvisy_ontology::modality::Modality::Strategy
//! [`Codable::Redaction`]: nvisy_codec::core::Codable
//! [`Redactions<M, R>`]: nvisy_codec::core::Redactions

use nvisy_codec::core::Redactions;
use nvisy_core::Result;
use nvisy_ontology::entity::EntityKind;
use nvisy_ontology::modality::{Mergeable, Modality, Overlap};
use nvisy_ontology::provenance::{AuditEntry, Execution};

use crate::envelope::DocumentEnvelope;
use crate::envelope::value_at::ValueAt;

const TARGET: &str = "nvisy_engine::redaction::apply";

/// Per-entry view passed to the per-modality `to_redaction`
/// converter. Carries everything a converter needs to produce the
/// codec wire type: the audit entry (for its strategy), the
/// entity's kind (for `{entityType}` placeholder substitution),
/// and the original value.
pub(super) struct EntryView<'a, M: Modality> {
    pub(super) entry: &'a AuditEntry<M>,
    pub(super) entity_kind: EntityKind,
    pub(super) original: &'a str,
}

/// One assembled per-modality batch ready to hand to the codec,
/// plus the side-tables the caller needs to commit audit state
/// after the codec accepts (or rejects) it.
pub(super) struct ApplyBatch<M: Modality, R> {
    pub(super) batch: Redactions<M, R>,
    pub(super) applied: Vec<(usize, R)>,
    pub(super) failed: Vec<(usize, String)>,
}

impl<M: Modality, R> ApplyBatch<M, R> {
    /// True when the batch produced no work — caller can short-
    /// circuit without touching the codec or the audit.
    pub(super) fn is_noop(&self) -> bool {
        self.applied.is_empty() && self.failed.is_empty()
    }
}

/// Assemble the per-modality redaction batch from the envelope's
/// pending records.
///
/// `to_redaction` converts an audit entry's strategy into the codec
/// wire type. Strategy errors mark the entry `Failed` without
/// aborting the batch.
///
/// The returned [`ApplyBatch`] holds the batch to submit plus the
/// per-record indices the caller will commit via [`commit`] once
/// the codec accepts the work.
pub(super) async fn build<M, R, F>(
    envelope: &DocumentEnvelope<M>,
    to_redaction: F,
) -> ApplyBatch<M, R>
where
    M: Modality + Overlap + Mergeable,
    R: Mergeable + Clone,
    F: Fn(EntryView<'_, M>) -> Result<R>,
    DocumentEnvelope<M>: ValueAt<M>,
{
    let pending: Vec<usize> = envelope
        .document
        .audit
        .records
        .iter()
        .enumerate()
        .filter(|(_, r)| r.audit.as_ref().is_some_and(|e| e.execution.is_pending()))
        .map(|(i, _)| i)
        .collect();

    if pending.is_empty() {
        return ApplyBatch {
            batch: Redactions::new(),
            applied: Vec::new(),
            failed: Vec::new(),
        };
    }

    let mut batch: Redactions<M, R> = Redactions::new();
    let mut applied: Vec<(usize, R)> = Vec::with_capacity(pending.len());
    let mut failed: Vec<(usize, String)> = Vec::new();

    for idx in pending {
        let record = &envelope.document.audit.records[idx];
        let entry = record
            .audit
            .as_ref()
            .expect("filtered to records with Some(audit) above");
        let entity = &record.entity;
        let original = envelope
            .value_at(&entity.location)
            .await
            .unwrap_or_default();
        let view = EntryView {
            entry,
            entity_kind: entity.entity_kind,
            original: &original,
        };
        let redaction = match to_redaction(view) {
            Ok(r) => r,
            Err(e) => {
                tracing::warn!(
                    target: TARGET,
                    entity_id = %entity.id,
                    error = %e,
                    "strategy conversion failed; marking entry Failed",
                );
                failed.push((idx, e.to_string()));
                continue;
            }
        };

        batch.insert(entity.location.clone(), redaction.clone());
        applied.push((idx, redaction));
    }

    ApplyBatch {
        batch,
        applied,
        failed,
    }
}

/// Commit per-record audit state once the codec has accepted the
/// batch produced by [`build`].
///
/// `to_replacement` is the per-modality hook that maps the codec
/// redaction back to the modality's `M::Replacement` shape — text/
/// tabular produce `TextReplacement` / `TabularReplacement`; image/
/// audio produce the `MethodTag` of the operation that ran.
pub(super) fn commit<M, R, ToReplacement>(
    envelope: &mut DocumentEnvelope<M>,
    applied: Vec<(usize, R)>,
    failed: Vec<(usize, String)>,
    to_replacement: ToReplacement,
) where
    M: Modality,
    ToReplacement: Fn(&R) -> M::Replacement,
{
    for (idx, redaction) in applied {
        let entry = envelope.document.audit.records[idx]
            .audit
            .as_mut()
            .expect("record had Some(audit) when build() ran");
        entry.execution = Execution::Applied {
            replacement: to_replacement(&redaction),
        };
    }
    for (idx, reason) in failed {
        let entry = envelope.document.audit.records[idx]
            .audit
            .as_mut()
            .expect("record had Some(audit) when build() ran");
        entry.execution = Execution::Failed { reason };
    }
}
