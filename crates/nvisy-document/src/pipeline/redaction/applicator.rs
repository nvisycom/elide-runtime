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
//! [`AnyAudit`]: crate::provenance::AnyAudit
//! [`AuditEntry`]: crate::provenance::AuditEntry
//! [`EntityRecord<M>`]: crate::provenance::EntityRecord
//! [`EntryMetadata::override_decision`]: crate::provenance::EntryMetadata::override_decision

use std::collections::HashMap;

use jiff::Timestamp;
use nvisy_core::Error;
use nvisy_core::entity::{Entity, EntityKind};
use nvisy_core::modality::{Audio, Image, Modality, Tabular, Text};

use crate::modality::AnyLocation;
use nvisy_core::primitive::Confidence;
use uuid::Uuid;

use super::override_::{RedactionAddEntity, RedactionOverride};
use crate::modality::DocumentModality;
use crate::policy::{Action, AnyRedaction};
use crate::provenance::{
    AnyAudit, Audit, AuditEntry, Decision, EntityRecord, EntryMetadata, Execution,
    RedactionDecision,
};

const TARGET: &str = "nvisy_document::pipeline::redaction::applicator";

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
    let mut consumed: HashMap<Uuid, ()> = HashMap::new();
    for any in audits.iter_mut() {
        match any {
            AnyAudit::Text(a) => apply_to::<Text>(a, &by_target, &mut consumed)?,
            AnyAudit::Tabular(a) => apply_to::<Tabular>(a, &by_target, &mut consumed)?,
            AnyAudit::Image(a) => apply_to::<Image>(a, &by_target, &mut consumed)?,
            AnyAudit::Audio(a) => apply_to::<Audio>(a, &by_target, &mut consumed)?,
        }
    }

    // Any targeted override that wasn't consumed is a typo or
    // stale id — fail loudly rather than silently drop.
    for target in by_target.keys() {
        if !consumed.contains_key(target) {
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
fn apply_to<M>(
    audit: &mut Audit<M>,
    overrides: &HashMap<Uuid, RedactionOverride>,
    consumed: &mut HashMap<Uuid, ()>,
) -> Result<(), Error>
where
    M: DocumentModality,
    M::Redaction: TryFromAnyRedaction,
{
    for record in &mut audit.records {
        let id = record.entity.id;
        let Some(ov) = overrides.get(&id) else {
            continue;
        };
        consumed.insert(id, ());
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
                let typed = M::Redaction::try_from_any(operator.clone()).ok_or_else(|| {
                    Error::validation(
                        format!(
                            "override Replace for entity {id} carries operator of modality {:?} but entity is modality {:?}",
                            operator.modality(),
                            modality_of::<M>(),
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
            (AnyAudit::Text(_), nvisy_core::modality::ModalityKind::Text)
                | (
                    AnyAudit::Tabular(_),
                    nvisy_core::modality::ModalityKind::Tabular
                )
                | (
                    AnyAudit::Image(_),
                    nvisy_core::modality::ModalityKind::Image
                )
                | (
                    AnyAudit::Audio(_),
                    nvisy_core::modality::ModalityKind::Audio
                )
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
        AnyAudit::Text(a) => append_typed::<Text>(a, entity_kind, location, operator)?,
        AnyAudit::Tabular(a) => append_typed::<Tabular>(a, entity_kind, location, operator)?,
        AnyAudit::Image(a) => append_typed::<Image>(a, entity_kind, location, operator)?,
        AnyAudit::Audio(a) => append_typed::<Audio>(a, entity_kind, location, operator)?,
    }
    Ok(())
}

/// Append a synthesised entity to a typed audit.
fn append_typed<M>(
    audit: &mut Audit<M>,
    entity_kind: EntityKind,
    location: AnyLocation,
    operator: Option<AnyRedaction>,
) -> Result<(), Error>
where
    M: DocumentModality + ExtractLocation,
    M::Redaction: TryFromAnyRedaction,
{
    let typed_loc = M::extract_location(location).ok_or_else(|| {
        // We checked modality above; this should be unreachable
        // but stays a typed error so future refactors don't
        // silently corrupt audits.
        Error::validation(
            "internal: append_typed called with mismatched modality",
            TARGET,
        )
    })?;
    let entity = Entity {
        id: Uuid::now_v7(),
        entity_id: None,
        entity_kind,
        location: typed_loc,
        confidence: Confidence::clamped(1.0),
        trail: Vec::new(),
        language: None,
    };
    let prebuilt = match operator {
        Some(op) => {
            let typed = M::Redaction::try_from_any(op).ok_or_else(|| {
                Error::validation(
                    "internal: append_typed operator modality mismatch (should be caught earlier)",
                    TARGET,
                )
            })?;
            Some(AuditEntry {
                decision: Decision {
                    policy_id: None,
                    rank: None,
                    action: Action::Redact { operator: typed },
                },
                execution: Execution::Pending,
                metadata: EntryMetadata {
                    timestamp: Some(Timestamp::now()),
                    correlation_id: None,
                    override_decision: Some(RedactionDecision::OverrideAdd),
                },
            })
        }
        None => None,
    };
    audit.records.push(EntityRecord {
        entity,
        audit: prebuilt,
    });
    Ok(())
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

/// Modality-aware projection from `AnyLocation` into `M::Location`.
trait ExtractLocation: Modality {
    fn extract_location(loc: AnyLocation) -> Option<Self::Location>;
}

impl ExtractLocation for Text {
    fn extract_location(loc: AnyLocation) -> Option<Self::Location> {
        match loc {
            AnyLocation::Text(l) => Some(l),
            _ => None,
        }
    }
}

impl ExtractLocation for Tabular {
    fn extract_location(loc: AnyLocation) -> Option<Self::Location> {
        match loc {
            AnyLocation::Tabular(l) => Some(l),
            _ => None,
        }
    }
}

impl ExtractLocation for Image {
    fn extract_location(loc: AnyLocation) -> Option<Self::Location> {
        match loc {
            AnyLocation::Image(l) => Some(l),
            _ => None,
        }
    }
}

impl ExtractLocation for Audio {
    fn extract_location(loc: AnyLocation) -> Option<Self::Location> {
        match loc {
            AnyLocation::Audio(l) => Some(l),
            _ => None,
        }
    }
}

/// Modality-aware projection from `AnyRedaction` into
/// `M::Redaction`.
trait TryFromAnyRedaction: Sized {
    fn try_from_any(any: AnyRedaction) -> Option<Self>;
}

impl TryFromAnyRedaction for crate::policy::redaction::TextRedaction {
    fn try_from_any(any: AnyRedaction) -> Option<Self> {
        match any {
            AnyRedaction::Text(r) => Some(r),
            _ => None,
        }
    }
}

impl TryFromAnyRedaction for crate::policy::redaction::TabularRedaction {
    fn try_from_any(any: AnyRedaction) -> Option<Self> {
        match any {
            AnyRedaction::Tabular(r) => Some(r),
            _ => None,
        }
    }
}

impl TryFromAnyRedaction for crate::policy::redaction::ImageRedaction {
    fn try_from_any(any: AnyRedaction) -> Option<Self> {
        match any {
            AnyRedaction::Image(r) => Some(r),
            _ => None,
        }
    }
}

impl TryFromAnyRedaction for crate::policy::redaction::AudioRedaction {
    fn try_from_any(any: AnyRedaction) -> Option<Self> {
        match any {
            AnyRedaction::Audio(r) => Some(r),
            _ => None,
        }
    }
}

/// Return the modality tag for a typed `M`.
fn modality_of<M: Modality>() -> nvisy_core::modality::ModalityKind {
    use nvisy_core::modality::ModalityKind;
    // We can't pattern-match on a type param; check via TypeId.
    let id = std::any::TypeId::of::<M>();
    if id == std::any::TypeId::of::<Text>() {
        ModalityKind::Text
    } else if id == std::any::TypeId::of::<Tabular>() {
        ModalityKind::Tabular
    } else if id == std::any::TypeId::of::<Image>() {
        ModalityKind::Image
    } else if id == std::any::TypeId::of::<Audio>() {
        ModalityKind::Audio
    } else {
        unreachable!("Modality must be one of Text/Tabular/Image/Audio");
    }
}
