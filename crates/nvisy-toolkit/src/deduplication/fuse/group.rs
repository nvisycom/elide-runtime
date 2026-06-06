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
use std::mem;

use nvisy_core::entity::{Entity, EntityKind};
use nvisy_core::extraction::TextAt;
use nvisy_core::modality::{Modality, Overlap};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::key::GroupKey;

const TARGET: &str = "nvisy_document::op::deduplication::group_entities";

/// How entity values and locations are matched when grouping.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
#[derive(Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum GroupingCriteria {
    /// Exact value match + overlapping location.
    #[default]
    Strict,
    /// Case-insensitive, trimmed value match + overlapping location.
    Normalized,
    /// Substring containment (shorter value is prefix/substring of longer)
    /// + overlapping location. Groups "John" with "John Smith".
    Narrowing,
    /// Same as narrowing but ignores location — groups the same entity
    /// across non-overlapping regions (e.g. cross-chunk deduplication).
    Widening,
}

impl GroupingCriteria {
    /// Whether two values match under this criteria.
    pub fn values_match(self, a: &str, b: &str) -> bool {
        match self {
            Self::Strict => a == b,
            Self::Normalized => a.trim().eq_ignore_ascii_case(b.trim()),
            Self::Narrowing | Self::Widening => {
                let (short, long) = if a.len() <= b.len() { (a, b) } else { (b, a) };
                long.contains(short)
            }
        }
    }

    /// Whether this criteria requires overlapping locations for grouping.
    pub fn requires_location_overlap(self) -> bool {
        !matches!(self, Self::Widening)
    }

    /// Whether this criteria uses substring containment for value matching.
    pub fn is_substring(self) -> bool {
        matches!(self, Self::Narrowing | Self::Widening)
    }

    /// Normalise a value for HashMap bucketing under this criteria.
    pub fn bucket_value(self, value: &str) -> String {
        match self {
            Self::Normalized => value.trim().to_lowercase(),
            _ => value.to_owned(),
        }
    }
}

/// Extension trait that groups entities for deduplication.
pub(super) trait GroupEntities<M: Modality> {
    /// Partition entities into groups of candidates that should be fused.
    ///
    /// Each returned `Vec<Entity>` contains entities that share the same
    /// kind/value (per the criteria) and overlap in location (unless the
    /// criteria ignores location).
    fn group<V: TextAt<M> + ?Sized>(
        self,
        criteria: GroupingCriteria,
        view: &V,
    ) -> impl Future<Output = Vec<Vec<Entity<M>>>> + Send;
}

impl<M> GroupEntities<M> for Vec<Entity<M>>
where
    M: Modality,
    M::Location: Overlap,
{
    async fn group<V: TextAt<M> + ?Sized>(
        self,
        criteria: GroupingCriteria,
        view: &V,
    ) -> Vec<Vec<Entity<M>>> {
        let check_overlap = criteria.requires_location_overlap();
        let is_substring = criteria.is_substring();
        let entity_count = self.len();

        // Phase 1: bucket by (kind, value).
        let mut buckets: HashMap<GroupKey, Vec<Entity<M>>> = HashMap::new();
        for entity in self {
            let key = GroupKey::new(&entity, criteria, view).await;
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
        let mut groups: Vec<Vec<Entity<M>>> = Vec::new();
        let mut kind_groups: HashMap<EntityKind, Vec<usize>> = HashMap::new();

        for (_key, bucket) in buckets {
            let kind = bucket[0].entity_kind;
            let mut sub_groups: Vec<Vec<Entity<M>>> = Vec::new();

            for entity in bucket {
                let bucket_match = sub_groups.iter_mut().find(|g| {
                    !check_overlap
                        || g.iter()
                            .any(|member| member.location.overlaps(&entity.location))
                });
                match bucket_match {
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
                        let mut any_value_match = false;
                        for a in &groups[indices[i]] {
                            for b in &groups[indices[j]] {
                                let va = view.text_at(&a.location).await;
                                let vb = view.text_at(&b.location).await;
                                if let (Some(va), Some(vb)) = (va.as_deref(), vb.as_deref())
                                    && criteria.values_match(va, vb)
                                {
                                    any_value_match = true;
                                    break;
                                }
                            }
                            if any_value_match {
                                break;
                            }
                        }
                        let location_ok = !check_overlap
                            || groups[indices[i]].iter().any(|a| {
                                groups[indices[j]]
                                    .iter()
                                    .any(|b| a.location.overlaps(&b.location))
                            });
                        if location_ok && any_value_match {
                            let donor = mem::take(&mut groups[indices[j]]);
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
