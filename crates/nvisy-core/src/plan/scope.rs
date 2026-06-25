//! Caller-asserted scope: languages + jurisdictions the recognizer
//! context carries.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::schema::LanguageTagSchema;

/// Per-request scope assertions. Engine threads these into every
/// recognizer's [`elide::recognition::Scope`].
#[derive(
    Debug,
    Clone,
    Default,
    PartialEq,
    Eq,
    Serialize,
    Deserialize,
    JsonSchema
)]
#[serde(rename_all = "camelCase")]
pub struct ScopeSpec {
    /// BCP 47 language tags the caller asserts (or has detected
    /// upstream). Each becomes an asserted [`Language`] in the
    /// recognizer context.
    ///
    /// [`Language`]: elide_core::primitive::Language
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub languages: Vec<LanguageTagSchema>,
    /// ISO 3166-1 alpha-2 country codes the caller asserts as
    /// applicable jurisdictions. Drives jurisdiction-scoped pattern
    /// packs in elide-pattern.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub jurisdictions: Vec<String>,
}
