//! Per-modality redaction applicator: materialises codec redaction
//! instructions from the [`EntityRecord`]s the evaluator decorated
//! with audit entries, and hands them off to the codec.
//!
//! One pass per envelope. The flow is the same for every modality:
//!
//! 1. Walk `document.audit.records`. For each record whose `audit`
//!    is present and `Pending` (i.e. not [`Suppressed`] and not
//!    already applied), read the entity's `location` and
//!    `entity_kind` directly off the record — no lookup needed.
//! 2. Convert the entry's [`Strategy`] into a codec-side
//!    [`Codable::Redaction`] via the per-modality strategy
//!    converter.
//! 3. Insert the `(M, Redaction)` pair into a
//!    [`Redactions<M, R>`] collection under the configured conflict
//!    policy.
//! 4. The per-modality caller hands the batch to the codec via the
//!    typed apply hook on the envelope, then commits per-entry
//!    audit state via [`commit`].
//!
//! [`EntityRecord`]: nvisy_ontology::provenance::EntityRecord
//! [`Suppressed`]: AuditEntryStatus::Suppressed
//! [`Strategy`]: nvisy_ontology::modality::Modality::Strategy
//! [`Codable::Redaction`]: nvisy_codec::core::Codable
//! [`Redactions<M, R>`]: nvisy_codec::core::Redactions

use nvisy_codec::core::{ConflictPolicy, Redactions};
use nvisy_core::Result;
use nvisy_ontology::entity::EntityKind;
use nvisy_ontology::modality::{Mergeable, Modality, Overlap};
use nvisy_ontology::provenance::{AuditEntry, AuditEntryStatus};

use crate::envelope::DocumentEnvelope;

const TARGET: &str = "nvisy_engine::redaction::apply";

/// Per-entry view passed to the per-modality `to_redaction`
/// converter. Carries everything a converter needs to produce the
/// codec wire type: the audit entry (for its strategy), the
/// entity's kind (for `{entityType}` placeholder substitution),
/// and the original value.
pub(super) struct EntryView<'a, M: Modality> {
    pub entry: &'a AuditEntry<M>,
    pub entity_kind: EntityKind,
    pub original: &'a str,
}

/// One assembled per-modality batch ready to hand to the codec,
/// plus the side-tables the caller needs to commit audit state
/// after the codec accepts (or rejects) it.
pub(super) struct ApplyBatch<M: Modality, R> {
    pub batch: Redactions<M, R>,
    pub applied: Vec<(usize, R)>,
    pub failed: Vec<usize>,
}

impl<M: Modality, R> ApplyBatch<M, R> {
    /// True when the batch produced no work — caller can short-
    /// circuit without touching the codec or the audit.
    pub fn is_noop(&self) -> bool {
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
pub(super) fn build<M, R, F>(
    envelope: &DocumentEnvelope<M>,
    to_redaction: F,
) -> ApplyBatch<M, R>
where
    M: Modality + Overlap + Mergeable,
    R: Mergeable + Clone,
    F: Fn(EntryView<'_, M>) -> Result<R>,
{
    let pending: Vec<usize> = envelope
        .document
        .audit
        .records
        .iter()
        .enumerate()
        .filter(|(_, r)| {
            r.audit
                .as_ref()
                .is_some_and(|e| e.status == AuditEntryStatus::Pending && !e.redaction.is_applied)
        })
        .map(|(i, _)| i)
        .collect();

    if pending.is_empty() {
        return ApplyBatch {
            batch: Redactions::new(ConflictPolicy::Merge),
            applied: Vec::new(),
            failed: Vec::new(),
        };
    }

    let mut batch: Redactions<M, R> = Redactions::new(ConflictPolicy::Merge);
    let mut applied: Vec<(usize, R)> = Vec::with_capacity(pending.len());
    let mut failed: Vec<usize> = Vec::new();

    for idx in pending {
        let record = &envelope.document.audit.records[idx];
        let entry = record
            .audit
            .as_ref()
            .expect("filtered to records with Some(audit) above");
        let entity = &record.entity;
        let original = entry.value.original.clone();
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
                failed.push(idx);
                continue;
            }
        };

        if let Err(e) = batch.try_insert(entity.location.clone(), redaction.clone()) {
            tracing::warn!(
                target: TARGET,
                entity_id = %entity.id,
                error = %e,
                "redaction conflicts with another in this batch; skipping",
            );
            failed.push(idx);
            continue;
        }
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
/// `on_success` is the per-modality hook that records a
/// modality-appropriate replacement on each accepted entry — text/
/// tabular fill in [`RedactionValue::replacement`] from the
/// codec's [`TextOutput`]; image/audio leave it unset.
///
/// [`RedactionValue::replacement`]: nvisy_ontology::provenance::RedactionValue::replacement
/// [`TextOutput`]: nvisy_codec::handler::TextOutput
pub(super) fn commit<M, R, OnSuccess>(
    envelope: &mut DocumentEnvelope<M>,
    applied: Vec<(usize, R)>,
    failed: Vec<usize>,
    on_success: OnSuccess,
) where
    M: Modality,
    OnSuccess: Fn(&mut AuditEntry<M>, &R),
{
    for (idx, redaction) in applied {
        let entry = envelope.document.audit.records[idx]
            .audit
            .as_mut()
            .expect("record had Some(audit) when build() ran");
        entry.redaction.is_applied = true;
        entry.status = AuditEntryStatus::Success;
        on_success(entry, &redaction);
    }
    for idx in failed {
        let entry = envelope.document.audit.records[idx]
            .audit
            .as_mut()
            .expect("record had Some(audit) when build() ran");
        entry.status = AuditEntryStatus::Failed;
    }
}

