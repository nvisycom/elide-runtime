//! [`PatternMetadata`]: pattern-level tags carried in the JSON's
//! `metadata` block.
//!
//! Every pattern may declare an optional `metadata` block alongside
//! `pattern` and `context`:
//!
//! ```json
//! {
//!   "name": "ssn",
//!   "category": "personal_identity",
//!   "entity_type": "government_id",
//!   "metadata": {
//!     "description": "US Social Security Number",
//!     "version": "1.0.0",
//!     "languages": ["en"],
//!     "regions": ["us"],
//!     "industries": ["healthcare", "finance", "government"],
//!     "compliance": ["hipaa", "ssn-protection"],
//!     "references": ["https://www.ssa.gov/employer/randomization.html"]
//!   },
//!   "pattern": { ... },
//!   "context": { ... }
//! }
//! ```
//!
//! Absent blocks default to [`PatternMetadata::default()`] (no tags,
//! version `0.0.0`).

use nvisy_ontology::primitive::LanguageTag;
use semver::Version;
use serde::{Deserialize, Serialize};

/// Pattern-level tags carried in the JSON's `metadata` block.
///
/// All fields are optional; absent fields parse as empty `Vec` (for
/// tag arrays) or `None` (for `description`). The `version` defaults
/// to `0.0.0` ("unversioned").
///
/// Tag semantics across multiple values in a single field is **OR**:
/// a pattern tagged `compliance: ["pci-dss", "hipaa"]` is applicable
/// when the consumer asks for PCI-DSS *or* HIPAA. Cross-field
/// semantics on the consumer side ([`PatternFilter`]) is **AND**.
///
/// An empty field is **unconstrained** — a pattern with no `languages`
/// tag passes any language filter (it's considered universally
/// applicable on that axis).
///
/// [`PatternFilter`]: crate::PatternFilter
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PatternMetadata {
    /// Free-form human description.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,

    /// Semver version of the pattern definition. Defaults to `0.0.0`
    /// when the metadata block omits it.
    #[serde(default = "default_version")]
    pub version: Version,

    /// BCP-47 language tags this pattern is applicable to.
    ///
    /// Multiple values mean the pattern applies for documents in any
    /// of the listed languages (OR). Empty means language-agnostic
    /// (passes any language filter).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub languages: Vec<LanguageTag>,

    /// Industry tags (free-form, lowercase convention).
    ///
    /// Examples: `"healthcare"`, `"legal"`, `"fintech"`. Empty means
    /// industry-agnostic.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub industries: Vec<String>,

    /// Region tags (free-form, ISO 3166 alpha-2 or `"global"`).
    ///
    /// Examples: `"us"`, `"eu"`, `"global"`. Empty means
    /// region-agnostic.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub regions: Vec<String>,

    /// Regulatory regime tags (free-form, lowercase convention).
    ///
    /// Examples: `"pci-dss"`, `"hipaa"`, `"gdpr"`, `"ccpa"`. Empty
    /// means no compliance regime claimed.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub compliance: Vec<String>,

    /// URLs to specs, regulations, or other provenance for the
    /// pattern. Audit-only; not consumed by the engine.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub references: Vec<String>,
}

fn default_version() -> Version {
    Version::new(0, 0, 0)
}

impl Default for PatternMetadata {
    fn default() -> Self {
        Self {
            description: None,
            version: default_version(),
            languages: Vec::new(),
            industries: Vec::new(),
            regions: Vec::new(),
            compliance: Vec::new(),
            references: Vec::new(),
        }
    }
}
