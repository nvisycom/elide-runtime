//! [`AuditContext`]: the recognition-side facts that travel with a
//! document from analyze to anonymize.
//!
//! Recorded when analysis runs and replayed when redaction does, so
//! the second pass compiles against exactly the vocabulary the
//! first one used.

use elide::primitive::{CountryCode, Languages, RasterMode};
use elide::recognition::ScopeMetadata;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::plan::scope_metadata_is_empty;

/// Recognition-side facts that travel from analyze to anonymize.
///
/// Mirrors elide's [`Scope`] shape one-for-one: direct fields
/// for `languages` and `countries` (typed, elide-native), a
/// [`metadata`] sub-struct for free-form classification strings
/// (`tags`, `purpose`, `audience`), and the analyze-time
/// [`correlation_id`]. The label catalog is not on here -
/// labels are policy-owned, and anonymize re-derives them from
/// the policy set it was handed.
///
/// No [`Default`]: `correlation_id` has no meaningful default
/// (a nil UUID would silently collapse unrelated audits under
/// one bucket in downstream trace aggregators), so callers
/// supply one explicitly. Everything else defaults to empty.
///
/// [`Scope`]: elide::recognition::Scope
/// [`metadata`]: Self::metadata
/// [`correlation_id`]: Self::correlation_id
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct AuditContext {
    /// Caller-asserted languages for the analysis.
    ///
    /// Recorded from `AnalyzerParams.scope.languages` at analyze
    /// time; anonymize re-uses them verbatim.
    #[serde(default)]
    pub languages: Languages,
    /// Caller-asserted jurisdictions.
    ///
    /// Recorded from `AnalyzerParams.scope.countries`.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub countries: Vec<CountryCode>,
    /// Free-form request context: document tags, request purpose,
    /// output audience. See elide's [`ScopeMetadata`].
    #[serde(default, skip_serializing_if = "scope_metadata_is_empty")]
    pub metadata: ScopeMetadata,
    /// Analyze-time correlation id.
    ///
    /// Threaded into every tracing span on the recognition path;
    /// carried over so the anonymize path can link its own spans
    /// to the same request. The anonymize call supplies a fresh
    /// id from the passed the analyzed document as the anonymize-side
    /// correlation id: this one stays as the analyze-side
    /// pointer.
    ///
    /// Required on the wire.
    ///
    pub correlation_id: Uuid,
    /// OCR mode the analyze call decoded with. Recorded so the
    /// anonymize call re-decodes the same document under the same
    /// codec configuration: otherwise entity offsets stored in
    /// the audit wouldn't line up against a differently-rendered
    /// second decode. Defaults to [`RasterMode::Auto`] (the codec's
    /// built-in behaviour) when omitted.
    #[serde(default)]
    pub raster_mode: RasterMode,
}
