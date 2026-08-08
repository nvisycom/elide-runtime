//! Predicates that gate whether a policy or rule fires.
//!
//! Two axes:
//!
//! - [`Predicate`] runs per entity; a rule fires only when its
//!   predicate holds against the candidate entity's facts (label,
//!   tag, confidence, coref).
//! - [`DocumentPredicate`] runs once per document; a policy is
//!   evaluated only when its `when` predicate holds
//!   against the document-level facts (labels, metadata,
//!   language, etc.).
//!
//! Both are serialisable. Engine compiles a `Predicate` into an
//! elide [`Rule`]: single-label predicates route through
//! [`Rule::label`], single-tag through [`Rule::tag`], and
//! everything else through [`Rule::predicate`] with a closure
//! over the [`MatchContext`]. Leaf variants inspect entity
//! facts; the composing variants ([`Predicate::All`],
//! [`Predicate::Any`], [`Predicate::Not`]) wire boolean algebra
//! over them.
//!
//! [`Rule`]: https://docs.rs/elide/latest/elide/redaction/struct.Rule.html
//! [`Rule::label`]: https://docs.rs/elide/latest/elide/redaction/struct.Rule.html#method.label
//! [`Rule::tag`]: https://docs.rs/elide/latest/elide/redaction/struct.Rule.html#method.tag
//! [`Rule::predicate`]: https://docs.rs/elide/latest/elide/redaction/struct.Rule.html#method.predicate
//! [`MatchContext`]: https://docs.rs/elide/latest/elide/redaction/struct.MatchContext.html

mod document;

use elide_core::entity::LabelRef;
use elide_core::primitive::ConfidenceThreshold;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

pub use self::document::DocumentPredicate;

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
    /// Entity label is in the named [`LabelGroup`] submitted
    /// alongside this request.
    ///
    /// Sugar over [`TagOneOf`]`{ tags: ["group:<name>"] }`: the
    /// engine synthesises a `group:<name>` tag on every label
    /// listed in the matching group at request-compile time, then
    /// rewrites `LabelInGroup { group }` to that `TagOneOf` form.
    /// Same fast path as any authored tag; the group indirection
    /// keeps the wire compact when templates target a canonical
    /// label cluster (e.g. `hipaa_18`, `gdpr_article_9`).
    ///
    /// An unknown group name is a request-validation error, not a
    /// silent no-op.
    ///
    /// [`LabelGroup`]: super::LabelGroup
    /// [`TagOneOf`]: Predicate::TagOneOf
    LabelInGroup {
        /// Name of the [`LabelGroup`] to match against.
        ///
        /// [`LabelGroup`]: super::LabelGroup
        group: String,
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
