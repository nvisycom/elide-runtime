//! Predicate that gates whether a rule fires against a
//! candidate entity.
//!
//! [`Predicate`] runs per entity; a rule fires only when its
//! predicate holds against the candidate entity's facts (label,
//! tag, confidence, coref).
//!
//! Engine compiles a `Predicate` into an elide [`Rule`]:
//! single-label predicates route through [`Rule::label`],
//! single-tag through [`Rule::tag`], and everything else through
//! [`Rule::predicate`] with a closure over the [`MatchContext`].
//! Leaf variants inspect entity facts; the composing variants
//! ([`Predicate::All`], [`Predicate::Any`], [`Predicate::Not`])
//! wire boolean algebra over them.
//!
//! [`Rule`]: https://docs.rs/elide/latest/elide/redaction/struct.Rule.html
//! [`Rule::label`]: https://docs.rs/elide/latest/elide/redaction/struct.Rule.html#method.label
//! [`Rule::tag`]: https://docs.rs/elide/latest/elide/redaction/struct.Rule.html#method.tag
//! [`Rule::predicate`]: https://docs.rs/elide/latest/elide/redaction/struct.Rule.html#method.predicate
//! [`MatchContext`]: https://docs.rs/elide/latest/elide/redaction/struct.MatchContext.html

use elide_core::entity::LabelRef;
use elide_core::primitive::ConfidenceThreshold;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Predicate over a recognised entity. The wire format uses an
/// internally tagged enum so authors write
/// `{ "kind": "confidence", "min": 0.7 }` etc.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum Predicate {
    /// Entity confidence is at least `min`.
    Confidence {
        /// Minimum confidence cutoff.
        min: ConfidenceThreshold,
    },
    /// Entity label is one of `labels`.
    LabelOneOf {
        /// Allowed labels.
        labels: Vec<LabelRef>,
    },
    /// Entity label carries one of `tags`, per the per-request
    /// label catalog.
    TagOneOf {
        /// Allowed tags.
        tags: Vec<String>,
    },
    /// Entity label is in the named [`LabelScope`] declared by
    /// the same [`PolicyDefinition`] this rule lives in.
    ///
    /// Only useful when a policy declares more than one scope, or
    /// when a rule should act on part of the vocabulary while
    /// [`fallback`] sweeps the rest. A policy with one scope whose
    /// rule targets all of it can drop the rule and let the
    /// fallback do the work.
    ///
    /// Scopes are policy-local: a rule cannot reference a scope
    /// declared by another policy in the same request. An unknown
    /// scope name is a request-validation error, not a silent
    /// no-op.
    ///
    /// [`LabelScope`]: super::LabelScope
    /// [`PolicyDefinition`]: super::PolicyDefinition
    /// [`fallback`]: super::PolicyDefinition::fallback
    LabelInScope {
        /// Name of the [`LabelScope`] to match against.
        ///
        /// [`LabelScope`]: super::LabelScope
        scope: String,
    },
    /// Entity carries the given coreference cluster id.
    CoRef {
        /// Cluster id to match.
        coref: String,
    },
    /// All sub-predicates must hold (AND).
    All {
        /// Conjunction members.
        all: Vec<Predicate>,
    },
    /// At least one sub-predicate must hold (OR).
    Any {
        /// Disjunction members.
        any: Vec<Predicate>,
    },
    /// Sub-predicate must not hold (NOT).
    Not {
        /// Negated predicate.
        not: Box<Predicate>,
    },
}
