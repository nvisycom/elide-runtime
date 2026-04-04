//! Two-phase entity grouping for deduplication.
//!
//! Groups entities that refer to the same real-world value so they
//! can be fused into a single detection. The algorithm runs in two
//! phases:
//!
//! 1. **Hash phase**: bucket entities by `(kind, value)` into a
//!    [`HashMap`] for O(n) partitioning.
//! 2. **Overlap phase**: within each bucket, sub-group by location
//!    overlap (O(k²) per bucket, where k is typically small). For
//!    substring criteria, a cross-bucket merge pass handles value
//!    containment across different hash keys.

use std::collections::{HashMap, HashSet};

use nvisy_ontology::entity::{Entities, Entity, EntityKind, Overlap};
use nvisy_ontology::workflow::GroupingCriteria;

const TARGET: &str = "nvisy_engine::op::deduplication::grouping";

/// Hash key for the first grouping phase.
///
/// For [`Strict`] and [`Narrowing`]/[`Widening`] this stores the exact
/// value; for [`Normalized`] it stores the lowercased, trimmed form.
///
/// [`Strict`]: GroupingCriteria::Strict
/// [`Narrowing`]: GroupingCriteria::Narrowing
/// [`Widening`]: GroupingCriteria::Widening
/// [`Normalized`]: GroupingCriteria::Normalized
#[derive(Hash, PartialEq, Eq)]
struct GroupKey {
    kind: EntityKind,
    value: String,
}

impl GroupKey {
    fn new(entity: &Entity, criteria: GroupingCriteria) -> Self {
        Self {
            kind: entity.entity_kind,
            value: criteria.bucket_value(entity.text_value().unwrap_or_default()),
        }
    }
}

/// Extension trait that groups entities for deduplication.
pub(super) trait GroupEntities {
    /// Partition entities into groups of candidates that should be fused.
    ///
    /// Each returned `Vec<Entity>` contains entities that share the same
    /// kind/value (per the criteria) and overlap in location (unless the
    /// criteria ignores location).
    fn group(self, criteria: GroupingCriteria) -> Vec<Vec<Entity>>;
}

impl GroupEntities for Entities {
    fn group(self, criteria: GroupingCriteria) -> Vec<Vec<Entity>> {
        let check_overlap = criteria.requires_location_overlap();
        let is_substring = criteria.is_substring();
        let entity_count = self.len();

        // Phase 1: bucket by (kind, value).
        let mut buckets: HashMap<GroupKey, Vec<Entity>> = HashMap::new();
        for entity in self {
            let key = GroupKey::new(&entity, criteria);
            buckets.entry(key).or_default().push(entity);
        }

        tracing::trace!(
            target: TARGET,
            entities = entity_count,
            buckets = buckets.len(),
            criteria = ?criteria,
            "hash phase complete",
        );

        // Phase 2a: within each bucket, sub-group by location overlap.
        let mut groups: Vec<Vec<Entity>> = Vec::new();
        let mut kind_groups: HashMap<EntityKind, Vec<usize>> = HashMap::new();

        for (_key, bucket) in buckets {
            let kind = bucket[0].entity_kind;
            let mut sub_groups: Vec<Vec<Entity>> = Vec::new();

            for entity in bucket {
                let target = sub_groups.iter_mut().find(|g| {
                    !check_overlap
                        || g.iter()
                            .any(|member| member.location.overlaps(&entity.location))
                });
                match target {
                    Some(g) => g.push(entity),
                    None => sub_groups.push(vec![entity]),
                }
            }

            for sg in sub_groups {
                let idx = groups.len();
                groups.push(sg);
                if is_substring {
                    kind_groups.entry(kind).or_default().push(idx);
                }
            }
        }

        // Phase 2b: for substring criteria, merge groups within the same
        // kind whose values have a containment relationship.
        if is_substring {
            let mut total_merges = 0usize;
            for indices in kind_groups.values() {
                let mut merged_into: HashSet<usize> = HashSet::new();
                for i in 0..indices.len() {
                    if merged_into.contains(&indices[i]) {
                        continue;
                    }
                    for j in (i + 1)..indices.len() {
                        if merged_into.contains(&indices[j]) {
                            continue;
                        }
                        let any_value_match = groups[indices[i]].iter().any(|a| {
                            groups[indices[j]].iter().any(|b| {
                                criteria.values_match(
                                    a.text_value().unwrap_or_default(),
                                    b.text_value().unwrap_or_default(),
                                )
                            })
                        });
                        let location_ok = !check_overlap
                            || groups[indices[i]].iter().any(|a| {
                                groups[indices[j]]
                                    .iter()
                                    .any(|b| a.location.overlaps(&b.location))
                            });
                        if location_ok && any_value_match {
                            let donor = std::mem::take(&mut groups[indices[j]]);
                            groups[indices[i]].extend(donor);
                            merged_into.insert(indices[j]);
                            total_merges += 1;
                        }
                    }
                }
            }
            groups.retain(|g| !g.is_empty());

            if total_merges > 0 {
                tracing::trace!(
                    target: TARGET,
                    total_merges,
                    "cross-bucket substring merge complete",
                );
            }
        }

        tracing::debug!(
            target: TARGET,
            entities = entity_count,
            groups = groups.len(),
            criteria = ?criteria,
            "grouping complete",
        );

        groups
    }
}
