//! Analyze → anonymize bridge: what [`Engine::analyze`] returns
//! and what [`Engine::anonymize`] accepts.
//!
//! [`Audit`] mirrors elide's [`Report`] shape: a body group +
//! zero-or-more container part groups (DOCX embedded images,
//! archive members, ...) keyed by container-private part id.
//! Every group is an [`EntityGroup`] tagged by modality so the
//! serialised form round-trips cleanly. Reviewer overrides live
//! per-entity inside [`EntityRecord`].
//!
//! [`AuditContext`] carries the recognition-side facts the
//! anonymize step needs to rebuild an orchestrator against the
//! exact vocabulary the analyze step used, minus the label
//! catalog: labels are policy-owned and re-derived from the
//! policy set on every anonymize call.
//!
//! Hosts hold this value between analyze and anonymize and may
//! persist it however they like (`serde` derives are on
//! everything).
//!
//! [`EntityGroup`]: crate::entity::EntityGroup
//! [`EntityRecord`]: crate::entity::EntityRecord
//! [`Engine::analyze`]: super::Engine::analyze
//! [`Engine::anonymize`]: super::Engine::anonymize
//! [`Report`]: elide::Report

use std::collections::HashMap;
#[cfg(feature = "audit-json")]
use std::io::Write;

use elide::primitive::{CountryCode, Languages, RasterMode};
use elide::recognition::{ScopeMetadata, UsageReport};
#[cfg(feature = "audit-json")]
use elide::{Error, ErrorKind, Result};
use elide_wire::plan::scope_metadata_is_empty;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::entity::EntityGroup;

/// What detection found in one document.
///
/// The body group plus per-container-part groups (each tagged by
/// modality) plus the recognition [`AuditContext`] the entities
/// were scored against.
///
/// The context travels with the entities so anonymize can rebuild
/// an orchestrator against exactly the vocabulary analyze used.
/// Anything a policy predicate compares against beyond the label
/// catalog (asserted languages, jurisdictions, document tags) is
/// here; labels are re-derived from the policy set on each
/// anonymize call.
///
/// No [`Default`]: a well-formed audit must carry a real
/// [`AuditContext`] with a real correlation id. Callers building
/// an audit outside the analyze path construct it explicitly.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct Audit {
    /// The body group.
    ///
    /// `None` when no body pipeline produced entities (pre-analyze,
    /// or the codec resolved the doc to a modality with no
    /// pipeline).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub body: Option<EntityGroup>,
    /// One entry per container part the orchestrator surfaced.
    ///
    /// Keyed by the container-private part id (e.g. a DOCX zip
    /// entry name like `"word/media/image1.png"`); each value
    /// carries that part's modality + entities.
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub parts: HashMap<String, EntityGroup>,
    /// Recognition context.
    ///
    /// The asserted languages, countries, document tags, and the
    /// analyze-side correlation id. Held so
    /// [`Engine::anonymize`] can compile against the same
    /// vocabulary analyze used without the caller re-passing an
    /// `AnalyzerParams`.
    ///
    /// Required on the wire: a missing context on an incoming
    /// [`Audit`] rejects at deserialize time so the shape
    /// mismatch surfaces at load, not at apply.
    ///
    /// [`Engine::anonymize`]: super::Engine::anonymize
    pub context: AuditContext,
    /// What the analyze pass cost: one entry per recognizer and
    /// enricher that ran, each self-identifying by the name the
    /// deployment configured it under.
    ///
    /// Empty when nothing model-backed ran, which is the common
    /// case for a pattern-only pass. Recorded on analyze and not
    /// re-derived at anonymize time, so a host that bills or rate
    /// limits on model spend reads it straight off the returned
    /// [`Audit`].
    #[serde(default, skip_serializing_if = "UsageReport::is_empty")]
    pub usage: UsageReport,
}

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
    /// id from the passed [`Document`] as the anonymize-side
    /// correlation id: this one stays as the analyze-side
    /// pointer.
    ///
    /// Required on the wire.
    ///
    /// [`Document`]: elide_wire::file::Document
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

#[cfg(feature = "audit-json")]
#[cfg_attr(docsrs, doc(cfg(feature = "audit-json")))]
impl Audit {
    /// Serialize the audit as pretty JSON into `writer`.
    ///
    /// Preserves the full structure: body + parts + context, and
    /// every entity's provenance chain. This is the canonical
    /// export; callers without a specific reason to reach for
    /// another format use this.
    ///
    /// # Errors
    ///
    /// Returns [`ErrorKind::Processing`] wrapping the underlying
    /// [`serde_json::Error`] (schema violation, I/O error on
    /// the writer).
    pub fn write_json<W: Write>(&self, writer: W) -> Result<()> {
        serde_json::to_writer_pretty(writer, self).map_err(|err| {
            Error::new(
                ErrorKind::Processing,
                format!("audit JSON export failed: {err}"),
            )
        })
    }
}
