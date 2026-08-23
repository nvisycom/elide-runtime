//! Analyze → anonymize bridge: what [`Engine::analyze`] returns
//! and what [`Engine::anonymize`] accepts.
//!
//! [`Audit`] wraps elide's [`Report`] — the detections, their
//! locations, and every entity's audit trail — with the three
//! things elide does not model: the recognition [`AuditContext`],
//! what the analyze pass cost, and the reviewer decisions in
//! [`ReviewSet`].
//!
//! Decisions sit beside the report rather than inside it, keyed by
//! entity id, because elide has no concept of a per-entity operator
//! override: apply re-resolves operators from live policy. The
//! decisions elide *does* model — adding an entity, suppressing one
//! — go on the report itself.
//!
//! [`AuditContext`] carries the recognition-side facts the
//! anonymize step needs to rebuild an orchestrator against the
//! exact vocabulary the analyze step used, minus the label
//! catalog: labels are policy-owned and re-derived from the
//! policy set on every anonymize call.
//!
//! Hosts hold this value between analyze and anonymize and may
//! persist it however they like: serialize it directly, and read it
//! back with [`Engine::deserialize_audit`].
//!
//! [`ReviewSet`]: crate::entity::ReviewSet
//! [`Engine::deserialize_audit`]: super::Engine::deserialize_audit
//! [`Engine::analyze`]: super::Engine::analyze
//! [`Engine::anonymize`]: super::Engine::anonymize
//! [`Report`]: elide::Report

use elide::Report;
use elide::primitive::{CountryCode, Languages, RasterMode};
use elide::recognition::{ScopeMetadata, UsageReport};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::entity::{Review, ReviewBucket, ReviewSet};
use crate::plan::scope_metadata_is_empty;

/// What detection found in one document, plus what a reviewer
/// decided about it.
///
/// Wraps elide's [`Report`] — the detections, their locations, and
/// every entity's audit trail — with the three things elide does
/// not model: the recognition [`AuditContext`] the entities were
/// scored against, what the analyze pass cost, and the reviewer
/// decisions in [`reviews`](Self::reviews).
///
/// The context travels with the entities so anonymize can rebuild
/// an orchestrator against exactly the vocabulary analyze used.
/// Anything a policy predicate compares against beyond the label
/// catalog (asserted languages, jurisdictions, document tags) is
/// here; labels are re-derived from the policy set on each
/// anonymize call.
///
/// # Serialization
///
/// [`Serialize`] but deliberately **not** `Deserialize`. A
/// serialized report tags each entity group with its modality
/// *name*, not its concrete type, and deserialization cannot be
/// object-safe — so rebuilding one needs the modality registry
/// [`Engine`] holds. Read an audit back with
/// [`Engine::deserialize_audit`]; a free `from_str` would need a
/// global registry, which would close the door on modalities elide
/// does not ship.
///
/// [`Engine`]: super::Engine
/// [`Engine::deserialize_audit`]: super::Engine::deserialize_audit
/// [`Report`]: elide::Report
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Audit {
    /// The detections: elide's own report, body and container
    /// parts, each entity carrying its provenance chain.
    ///
    /// Edit it through [`Report`]'s own API — [`include`],
    /// [`suppress`], [`entities`] — for the decisions elide models.
    /// Operator overrides live in [`reviews`](Self::reviews).
    ///
    /// [`Report`]: elide::Report
    /// [`include`]: elide::Report::include
    /// [`suppress`]: elide::Report::suppress
    /// [`entities`]: elide::Report::entities
    pub report: Report,
    /// What a reviewer decided about individual detections, keyed
    /// by entity id.
    ///
    /// Separate from the report because elide has no concept of a
    /// per-entity operator override: [`anonymize_with`] re-resolves
    /// operators from live policy at apply time.
    ///
    /// [`anonymize_with`]: elide::Orchestrator::anonymize_with
    #[serde(skip_serializing_if = "ReviewSet::is_empty")]
    pub reviews: ReviewSet,
    /// Recognition context.
    ///
    /// The asserted languages, countries, document tags, and the
    /// analyze-side correlation id. Held so [`Engine::anonymize`]
    /// can compile against the same vocabulary analyze used
    /// without the caller re-passing an `AnalyzerParams`.
    ///
    /// [`Engine::anonymize`]: super::Engine::anonymize
    pub context: AuditContext,
    /// What the analyze pass cost: one entry per recognizer and
    /// enricher that ran, each self-identifying by the name the
    /// deployment configured it under.
    ///
    /// Carried here rather than read off the report: elide derives
    /// usage during analysis and drops it when a report is rebuilt
    /// from the wire, so a host that bills on model spend would
    /// lose it on the round trip.
    #[serde(skip_serializing_if = "UsageReport::is_empty")]
    pub usage: UsageReport,
}

impl Audit {
    /// Record a reviewer's decision about the entity `id`.
    ///
    /// Replaces any decision already held for it: one entity
    /// carries one decision. The modality is the entity's own, so a
    /// text entity can only be given a [`TextRedaction`] — a review
    /// naming the wrong modality's operator will not compile.
    ///
    /// [`TextRedaction`]: elide_governance::redaction::TextRedaction
    pub fn review<M: ReviewBucket>(&mut self, id: Uuid, review: Review<M>) {
        M::bucket_mut(&mut self.reviews).insert(id, review);
    }

    /// The decision held for the entity `id`, if any.
    #[must_use]
    pub fn reviewed<M: ReviewBucket>(&self, id: Uuid) -> Option<&Review<M>> {
        M::bucket(&self.reviews).get(&id)
    }

    /// Drop any decision held for the entity `id`, restoring it to
    /// whatever the policy set picks.
    pub fn unreview<M: ReviewBucket>(&mut self, id: Uuid) -> Option<Review<M>> {
        M::bucket_mut(&mut self.reviews).remove(&id)
    }

    /// Whether the entity `id` will be left alone.
    ///
    /// A pending decision wins over the entity's trail, so an entity
    /// suppressed, applied, then given a [`Review::Redact`] reads
    /// `false` here and is redacted on the next apply. With no
    /// pending decision this falls back to the trail, so an applied
    /// suppression still reads as suppressed after a round trip.
    #[must_use]
    pub fn is_suppressed<M: ReviewBucket + 'static>(&self, id: Uuid) -> bool {
        match self.reviewed::<M>(id) {
            Some(Review::Suppress { .. }) => true,
            Some(Review::Redact { .. } | Review::Retag { .. }) => false,
            None => self
                .report
                .entities::<M>()
                .and_then(|entities| entities.iter().find(|e| e.id == id))
                .is_some_and(elide::entity::Entity::is_suppressed),
        }
    }
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
    /// [`Document`]: crate::file::Document
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
