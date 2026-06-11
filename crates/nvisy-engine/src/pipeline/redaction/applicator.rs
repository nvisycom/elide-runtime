//! [`OverrideApplicator`]: apply human overrides to a detection's
//! audit before the redaction phase consumes it.
//!
//! Inputs: an immutable [`AnyAudit`] from the detection +
//! every override targeting it. Output: a mutated `AnyAudit`
//! ready for the redaction phase. Provenance for each override
//! is stamped onto the produced [`EntryMetadata::override_decision`]
//! so reviewers see "what happened and why."
//!
//! Order of operations per audit:
//!
//! 1. Apply `Accept` / `Reject` / `Replace` overrides against
//!    existing records (lookup by `Entity::id`).
//! 2. For each `Add` override whose `location.kind()` matches
//!    this audit's modality, synthesise an
//!    [`EntityRecord<M>`] with a fresh UUID and append it. The
//!    redaction phase's normal policy-resolution path runs the
//!    chain for it; if the override pinned an operator, the
//!    record arrives with a pre-resolved [`AuditEntry`] and the
//!    redaction phase honours it directly.
//!
//! [`AnyAudit`]: crate::document::provenance::AnyAudit
//! [`AuditEntry`]: crate::document::provenance::AuditEntry
//! [`EntityRecord<M>`]: crate::document::provenance::EntityRecord
//! [`EntryMetadata::override_decision`]: crate::document::provenance::EntryMetadata::override_decision

use std::collections::{HashMap, HashSet};

use jiff::Timestamp;
use nvisy_core::Error;
use nvisy_core::entity::{Entity, EntityKind};
use nvisy_core::modality::ModalityKind;
use nvisy_core::primitive::Confidence;
use uuid::Uuid;

use super::override_::{RedactionAddEntity, RedactionOverride};
use crate::document::provenance::{
    AnyAudit, Audit, AuditEntry, Decision, EntityRecord, EntryMetadata, Execution,
    RedactionDecision,
};
use crate::modality::DocumentModality;
use crate::policy::{Action, AnyRedaction};

const TARGET: &str = "nvisy_engine::pipeline::redaction::applicator";

/// Apply a batch of overrides to a vector of audits.
///
/// Mutates `audits` in place. Returns an error when any override
/// references an entity-id that isn't present in any of the
/// audits — silently dropping such an override would let typos
/// bypass redaction without anyone noticing.
///
/// # Errors
///
/// - [`ErrorKind::Validation`] when an override's
///   `entity_id` doesn't resolve to any entity in the detection.
/// - [`ErrorKind::Validation`] when a `Replace` operator's
///   modality differs from the targeted entity's modality.
/// - [`ErrorKind::Validation`] when an `Add` override's pinned
///   operator modality differs from the location's modality
///   (also caught earlier by `validate_overrides`; double-checked
///   here as defence-in-depth).
///
/// [`ErrorKind::Validation`]: nvisy_core::ErrorKind::Validation
pub(crate) fn apply_overrides(
    audits: &mut [AnyAudit],
    overrides: Vec<RedactionOverride>,
) -> Result<(), Error> {
    // Bucket overrides by target so we can validate "every
    // entity-id-targeted override resolved" cheaply at the end.
    let mut by_target: HashMap<Uuid, RedactionOverride> = HashMap::new();
    let mut adds: Vec<RedactionAddEntity> = Vec::new();
    for ov in overrides {
        match ov {
            RedactionOverride::Add(add) => adds.push(add),
            other => {
                let Some(target) = other.target() else {
                    unreachable!("only Add has no target; matched above");
                };
                if let Some(existing) = by_target.insert(target, other) {
                    return Err(Error::validation(
                        format!(
                            "duplicate override for entity {target} (had {existing:?})",
                            existing = std::mem::discriminant(&existing),
                        ),
                        TARGET,
                    ));
                }
            }
        }
    }

    // Apply per-audit. Track which targeted overrides were
    // consumed so unresolved ones surface as errors.
    let mut consumed: HashSet<Uuid> = HashSet::new();
    for any in audits.iter_mut() {
        match any {
            AnyAudit::Text(a) => apply_to(a, &by_target, &mut consumed, AnyRedaction::try_as_text)?,
            AnyAudit::Tabular(a) => {
                apply_to(a, &by_target, &mut consumed, AnyRedaction::try_as_tabular)?
            }
            AnyAudit::Image(a) => {
                apply_to(a, &by_target, &mut consumed, AnyRedaction::try_as_image)?
            }
            AnyAudit::Audio(a) => {
                apply_to(a, &by_target, &mut consumed, AnyRedaction::try_as_audio)?
            }
        }
    }

    // Any targeted override that wasn't consumed is a typo or
    // stale id — fail loudly rather than silently drop.
    for target in by_target.keys() {
        if !consumed.contains(target) {
            return Err(Error::validation(
                format!("override targets entity {target} not present in detection"),
                TARGET,
            ));
        }
    }

    // Apply `Add` overrides last. Match each `AnyLocation` variant
    // to the corresponding audit (first audit of matching
    // modality) so the synthesised entity lands in the right
    // document. If no audit of matching modality exists, fail —
    // we can't redact bytes that aren't in the run.
    for add in adds {
        append_add(audits, add)?;
    }
    Ok(())
}

/// Apply Accept/Reject/Replace overrides to one typed audit.
/// Records consumed override targets in `consumed`.
///
/// `project_operator` is the per-modality `AnyRedaction::try_as_*`
/// the caller supplies based on `M`; keeps the helper generic
/// without a `TryFrom<AnyRedaction>` trait bound on
/// `M::Redaction`.
fn apply_to<M, F>(
    audit: &mut Audit<M>,
    overrides: &HashMap<Uuid, RedactionOverride>,
    consumed: &mut HashSet<Uuid>,
    project_operator: F,
) -> Result<(), Error>
where
    M: DocumentModality,
    F: Fn(AnyRedaction) -> Option<M::Redaction>,
{
    for record in &mut audit.records {
        let id = record.entity.id;
        let Some(ov) = overrides.get(&id) else {
            continue;
        };
        consumed.insert(id);
        match ov {
            RedactionOverride::Accept { .. } => {
                stamp_provenance(record, RedactionDecision::OverrideAccept);
            }
            RedactionOverride::Reject { .. } => {
                let entry = record.audit.take().unwrap_or_else(|| AuditEntry {
                    decision: Decision {
                        policy_id: None,
                        rank: None,
                        action: Action::Suppress,
                    },
                    execution: Execution::Suppressed,
                    metadata: EntryMetadata::now().with_override(RedactionDecision::OverrideReject),
                });
                let mut entry = entry;
                entry.execution = Execution::Suppressed;
                entry.metadata = entry
                    .metadata
                    .with_override(RedactionDecision::OverrideReject);
                record.audit = Some(entry);
            }
            RedactionOverride::Replace {
                entity_id: _,
                operator,
            } => {
                let typed = project_operator(operator.clone()).ok_or_else(|| {
                    Error::validation(
                        format!(
                            "override Replace for entity {id} carries operator of modality {:?} but entity is modality {:?}",
                            operator.modality(),
                            ModalityKind::of::<M>(),
                        ),
                        TARGET,
                    )
                })?;
                let prior = record.audit.take();
                let prior_decision = prior
                    .as_ref()
                    .map(|e| (e.decision.policy_id, e.decision.rank));
                let (policy_id, rank) = prior_decision.unwrap_or((None, None));
                record.audit = Some(AuditEntry {
                    decision: Decision {
                        policy_id,
                        rank,
                        action: Action::Redact { operator: typed },
                    },
                    execution: Execution::Pending,
                    metadata: EntryMetadata::now()
                        .with_override(RedactionDecision::OverrideReplace),
                });
            }
            RedactionOverride::Add(_) => {
                unreachable!("Add overrides handled separately in append_add")
            }
        }
    }
    Ok(())
}

/// Append a synthesised entity to the matching audit.
fn append_add(audits: &mut [AnyAudit], add: RedactionAddEntity) -> Result<(), Error> {
    // Optional pinned operator: if it disagrees with the
    // location's modality, fail.
    if let Some(op) = &add.operator
        && op.modality() != add.location.kind()
    {
        return Err(Error::validation(
            format!(
                "override Add operator modality {:?} differs from location modality {:?}",
                op.modality(),
                add.location.kind(),
            ),
            TARGET,
        ));
    }

    // Find a target audit of matching modality.
    let kind = add.location.kind();
    let target = audits.iter_mut().find(|a| {
        matches!(
            (a, kind),
            (AnyAudit::Text(_), ModalityKind::Text)
                | (AnyAudit::Tabular(_), ModalityKind::Tabular)
                | (AnyAudit::Image(_), ModalityKind::Image)
                | (AnyAudit::Audio(_), ModalityKind::Audio)
        )
    });
    let Some(target) = target else {
        return Err(Error::validation(
            format!(
                "override Add for modality {:?} has no audit of that modality in the detection",
                add.location.kind(),
            ),
            TARGET,
        ));
    };

    let entity_kind = add.entity_kind;
    let operator = add.operator;
    let location = add.location;
    match target {
        AnyAudit::Text(a) => {
            let loc = location.try_as_text().ok_or_else(modality_mismatch)?;
            let op = operator
                .map(|o| o.try_as_text().ok_or_else(modality_mismatch))
                .transpose()?;
            append_typed(a, entity_kind, loc, op);
        }
        AnyAudit::Tabular(a) => {
            let loc = location.try_as_tabular().ok_or_else(modality_mismatch)?;
            let op = operator
                .map(|o| o.try_as_tabular().ok_or_else(modality_mismatch))
                .transpose()?;
            append_typed(a, entity_kind, loc, op);
        }
        AnyAudit::Image(a) => {
            let loc = location.try_as_image().ok_or_else(modality_mismatch)?;
            let op = operator
                .map(|o| o.try_as_image().ok_or_else(modality_mismatch))
                .transpose()?;
            append_typed(a, entity_kind, loc, op);
        }
        AnyAudit::Audio(a) => {
            let loc = location.try_as_audio().ok_or_else(modality_mismatch)?;
            let op = operator
                .map(|o| o.try_as_audio().ok_or_else(modality_mismatch))
                .transpose()?;
            append_typed(a, entity_kind, loc, op);
        }
    }
    Ok(())
}

fn modality_mismatch() -> Error {
    // Caught earlier by `validate_overrides` and the kind-match
    // above; double-checked here as defence-in-depth.
    Error::validation(
        "internal: append_typed called with mismatched modality",
        TARGET,
    )
}

/// Append a synthesised entity to a typed audit.
fn append_typed<M>(
    audit: &mut Audit<M>,
    entity_kind: EntityKind,
    location: M::Location,
    operator: Option<M::Redaction>,
) where
    M: DocumentModality,
{
    let entity = Entity {
        id: Uuid::now_v7(),
        entity_id: None,
        entity_kind,
        location,
        confidence: Confidence::clamped(1.0),
        trail: Vec::new(),
        language: None,
    };
    let prebuilt = operator.map(|op| AuditEntry {
        decision: Decision {
            policy_id: None,
            rank: None,
            action: Action::Redact { operator: op },
        },
        execution: Execution::Pending,
        metadata: EntryMetadata {
            timestamp: Some(Timestamp::now()),
            correlation_id: None,
            override_decision: Some(RedactionDecision::OverrideAdd),
        },
    });
    audit.records.push(EntityRecord {
        entity,
        audit: prebuilt,
    });
}

/// Stamp `record.audit.metadata.override_decision` with `tag`.
/// If the record has no audit entry yet, none is created — the
/// redaction phase will fill it in via policy resolution; the
/// override tag stays "OverrideAccept" applied at that point.
fn stamp_provenance<M>(record: &mut EntityRecord<M>, tag: RedactionDecision)
where
    M: DocumentModality,
{
    if let Some(entry) = record.audit.as_mut() {
        entry.metadata = entry.metadata.clone().with_override(tag);
    } else {
        record.audit = Some(AuditEntry {
            decision: Decision {
                policy_id: None,
                rank: None,
                action: Action::Suppress,
            },
            execution: Execution::Pending,
            metadata: EntryMetadata::now().with_override(tag),
        });
    }
}
