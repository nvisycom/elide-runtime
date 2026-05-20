//! [`DictionaryMetadata`]: per-dictionary tags loaded from `name.json` sidecars.
//!
//! Each dictionary file may have an optional sidecar JSON with the same
//! stem (e.g. `nationalities.txt` + `nationalities.json`) carrying
//! language/industry/region tags and a free-form description. The
//! engine uses these tags to filter the active dictionary set per scan;
//! see [`PatternDetection::dictionary_filter`].
//!
//! When the sidecar is absent the dictionary loads with
//! [`DictionaryMetadata::default()`] (no tags).
//!
//! [`PatternDetection::dictionary_filter`]: nvisy_ontology::workflow::PatternDetection

use nvisy_ontology::primitive::LanguageTag;
use semver::Version;
use serde::{Deserialize, Serialize};

/// Per-dictionary tags loaded from a `name.json` sidecar file.
///
/// All fields are optional; absent fields parse as empty `Vec` (for
/// tag arrays) or `None` (for `description`). The `version` defaults
/// to `0.0.0` ("unversioned").
///
/// Tag semantics across multiple values in a single field is **OR**:
/// a dictionary tagged `languages: ["en", "de"]` is applicable when
/// the consumer asks for English *or* German.
///
/// Cross-field semantics on the consumer side ([`PatternFilter`])
/// is **AND**: a filter `{ languages: ["en"], industries: ["healthcare"] }`
/// selects dictionaries that have English in `languages` *and* `healthcare`
/// in `industries`.
///
/// [`PatternFilter`]: nvisy_ontology::workflow::PatternFilter
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DictionaryMetadata {
    /// Override the dictionary's name in the registry. When present
    /// this wins verbatim over the path-derived default; absence keeps
    /// the path-based name (relative path without extension, e.g.
    /// `healthcare/drugs`).
    ///
    /// Use this to keep a stable short name (`currencies`) even after
    /// moving the file into a subfolder (`finance/currencies.csv`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,

    /// Free-form human description.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,

    /// Semver version of the dictionary content. Defaults to `0.0.0`
    /// (unversioned) when the sidecar omits it.
    #[serde(default = "default_version")]
    pub version: Version,

    /// BCP-47 language tags this dictionary is applicable to.
    ///
    /// Multiple values mean the dictionary is applicable for documents
    /// in any of the listed languages (OR). Empty means language-agnostic.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub languages: Vec<LanguageTag>,

    /// Industry tags (free-form, lowercase convention).
    ///
    /// Examples: `"healthcare"`, `"legal"`, `"fintech"`. Empty means
    /// industry-agnostic.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub industries: Vec<String>,

    /// Region tags (free-form, ISO 3166-2 or `"global"` by convention).
    ///
    /// Examples: `"us"`, `"eu"`, `"global"`. Empty means region-agnostic.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub regions: Vec<String>,

    /// Regulatory regime tags (free-form, lowercase convention).
    ///
    /// Examples: `"pci-dss"`, `"hipaa"`, `"gdpr"`, `"ccpa"`. Empty
    /// means no compliance regime claimed.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub compliance: Vec<String>,
}

fn default_version() -> Version {
    Version::new(0, 0, 0)
}

impl Default for DictionaryMetadata {
    fn default() -> Self {
        Self {
            name: None,
            description: None,
            version: default_version(),
            languages: Vec::new(),
            industries: Vec::new(),
            regions: Vec::new(),
            compliance: Vec::new(),
        }
    }
}
