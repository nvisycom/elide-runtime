//! [`DocumentContext`]: what a caller asserts about a document,
//! from analyze through anonymize.
//!
//! One type for both ends of the round trip. A caller passes it to
//! analyze; the audit carries it back so anonymize compiles against
//! exactly the vocabulary the first pass used. It was two types —
//! request input and recorded state — but the two roles never
//! diverged: anonymize needs precisely what analyze was asked for,
//! and a second type only added a field-for-field copy between
//! them.
//!
//! Mirrors elide's own [`Scope`]: `languages` and `countries` flat
//! and typed, free-form classification strings nested under
//! [`metadata`]. Three of `Scope`'s fields are deliberately absent.
//! The label catalog is policy-owned, re-derived from the policy set
//! on every call. The correlation id belongs to the document, which
//! every call already takes. How the document *decodes* is
//! [`CodecParams`], because that feeds the codec rather than
//! recognition.
//!
//! [`Scope`]: elide::recognition::Scope
//! [`CodecParams`]: super::CodecParams
//! [`metadata`]: DocumentContext::metadata

use elide::primitive::{CountryCode, Languages};
use elide::recognition::ScopeMetadata;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// What a caller asserts about the document being processed.
///
/// Everything defaults to empty, so a caller asserting nothing
/// passes [`DocumentContext::default`] and lets the recognizers use
/// their own defaults.
#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", default)]
pub struct DocumentContext {
    /// Languages the caller asserts the document is in.
    ///
    /// Recognizers that take a language hint use it; the
    /// language-detection enricher fills the gap when this is
    /// empty.
    pub languages: Languages,
    /// Jurisdictions the caller asserts apply.
    ///
    /// Read by policy predicates that vary by jurisdiction, so a
    /// rule can act on a document from one country and not another.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub countries: Vec<CountryCode>,
    /// Free-form request context: document tags, request purpose,
    /// output audience. See elide's [`ScopeMetadata`].
    #[serde(skip_serializing_if = "ScopeMetadata::is_empty")]
    pub metadata: ScopeMetadata,
}
