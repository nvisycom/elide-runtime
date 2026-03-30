//! Two-phase entity grouping for fusion and deduplication.

use std::collections::{HashMap, HashSet};

use nvisy_ontology::entity::{Entities, Entity, EntityKind, Overlap};
use nvisy_ontology::workflow::GroupingCriteria;

/// Hash key for the first grouping phase.
#[derive(Hash, PartialEq, Eq)]
struct GroupKey {
    kind: EntityKind,
    value: String,
}

impl GroupKey {
    fn new(entity: &Entity, criteria: GroupingCriteria) -> Self {
        Self {
            kind: entity.entity_kind,
            value: criteria.bucket_value(&entity.value),
        }
    }
}

/// Extension trait that groups entities for fusion.
pub(super) trait GroupEntities {
    /// Group entities using a two-phase approach:
    ///
    /// 1. **Hash phase** — bucket by `(kind, normalized_value)` via HashMap.
    /// 2. **Merge phase** — within each bucket, merge groups by value match
    ///    + optional location overlap.
    ///
    /// For substring criteria (Narrowing/Widening), entities with different
    /// exact values land in different buckets. A post-hash cross-bucket
    /// merge handles substring containment within the same EntityKind.
    fn group(self, criteria: GroupingCriteria) -> Vec<Vec<Entity>>;
}

impl GroupEntities for Entities {
    fn group(self, criteria: GroupingCriteria) -> Vec<Vec<Entity>> {
        let check_overlap = criteria.requires_location_overlap();
        let is_substring = criteria.is_substring();

        // Phase 1: bucket by (kind, value).
        let mut buckets: HashMap<GroupKey, Vec<Entity>> = HashMap::new();
        for entity in self {
            let key = GroupKey::new(&entity, criteria);
            buckets.entry(key).or_default().push(entity);
        }

        // Phase 2a: within each bucket, sub-group by location overlap.
        let mut groups: Vec<Vec<Entity>> = Vec::new();
        let mut kind_groups: HashMap<EntityKind, Vec<usize>> = HashMap::new();

        for (_key, bucket) in buckets {
            let kind = bucket[0].entity_kind;
            let mut sub_groups: Vec<Vec<Entity>> = Vec::new();

            for entity in bucket {
                let target = sub_groups
                    .iter_mut()
                    .find(|g| !check_overlap || g[0].location.overlaps(&entity.location));
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
                        let val_a = &groups[indices[i]][0].value;
                        let val_b = &groups[indices[j]][0].value;
                        let location_ok = !check_overlap
                            || groups[indices[i]][0]
                                .location
                                .overlaps(&groups[indices[j]][0].location);
                        if location_ok && criteria.values_match(val_a, val_b) {
                            let donor = std::mem::take(&mut groups[indices[j]]);
                            groups[indices[i]].extend(donor);
                            merged_into.insert(indices[j]);
                        }
                    }
                }
            }
            groups.retain(|g| !g.is_empty());
        }

        groups
    }
}
