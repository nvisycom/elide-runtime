//! Document-level predicate that gates whether a whole policy
//! applies to a given document.
//!
//! Evaluated once per document (cf. [`Predicate`], which is
//! evaluated per entity). Engine merges the document's content
//! descriptor with the caller's per-request metadata, then checks
//! the predicate against that union.
//!
//! [`Predicate`]: super::Predicate

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Predicate over document-level facts.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum DocumentPredicate {
    /// Document carries `label` (e.g. `"contract"`, `"medical"`).
    /// Labels come from the content descriptor; the importer
    /// populates them at ingest time.
    HasLabel {
        /// Required document label.
        label: String,
    },
    /// Document metadata has `key` set, optionally to `value`. When
    /// `value` is `None` the key just has to be present.
    HasMetadata {
        /// Metadata key to read.
        key: String,
        /// Required value, or `None` to match on presence alone.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        value: Option<String>,
    },
    /// All sub-predicates must hold (AND).
    All {
        /// Conjunction members.
        all: Vec<DocumentPredicate>,
    },
    /// At least one sub-predicate must hold (OR).
    Any {
        /// Disjunction members.
        any: Vec<DocumentPredicate>,
    },
    /// Sub-predicate must not hold (NOT).
    Not {
        /// Negated predicate.
        not: Box<DocumentPredicate>,
    },
}
