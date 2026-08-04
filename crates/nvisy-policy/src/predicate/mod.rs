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
//! Both are serialisable. Engine compiles a `Predicate` into a
//! closure passed to
//! `elide::redaction::Anonymizer::with_catalog_predicate` (or
//! routed to [`with_label`] / [`with_tag`] fast paths for the
//! degenerate single-label / single-tag shapes). Leaf variants
//! inspect entity facts; the composing variants
//! ([`Predicate::All`], [`Predicate::Any`], [`Predicate::Not`])
//! wire boolean algebra over them.
//!
//! [`with_label`]: https://docs.rs/elide/latest/elide/redaction/Anonymizer::with_label
//! [`with_tag`]: https://docs.rs/elide/latest/elide/redaction/Anonymizer::with_tag

mod document;

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
        labels: Vec<String>,
    },
    /// Entity label carries one of `tags`, per the per-request
    /// label catalog.
    TagOneOf {
        /// Allowed tags.
        tags: Vec<String>,
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
