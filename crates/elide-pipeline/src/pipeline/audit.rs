//! Analyze → anonymize bridge: what [`Engine::analyze`] returns
//! and what [`Engine::anonymize`] accepts.
//!
//! Reviewer decisions sit beside the report rather than inside it,
//! keyed by entity id, because elide has no concept of a per-entity
//! operator override: apply re-resolves operators from live policy.
//! The decisions elide *does* model — adding an entity, suppressing
//! one — go on the report itself.
//!
//! Hosts hold an [`Audit`] between the two passes and may persist
//! it however they like: serialize it directly, and read it back
//! with [`Engine::deserialize_audit`].
//!
//! [`Engine::analyze`]: super::Engine::analyze
//! [`Engine::anonymize`]: super::Engine::anonymize
//! [`Engine::deserialize_audit`]: super::Engine::deserialize_audit

use elide::Report;
use elide::recognition::UsageReport;
use elide_provider::{CodecParams, DocumentContext};
use schemars::JsonSchema;
use serde::Serialize;
use uuid::Uuid;

use crate::entity::{Review, ReviewBucket, ReviewSet};

/// What detection found in one document, plus what a reviewer
/// decided about it.
///
/// Wraps elide's [`Report`] with the three things elide does not
/// model: the recognition [`DocumentContext`] the entities were
/// scored against, how the document decoded, and the reviewer
/// decisions in [`reviews`](Self::reviews).
///
/// # Serialization
///
/// [`Serialize`] but deliberately **not** `Deserialize`: a
/// serialized report tags entity groups by modality *name*, so
/// rebuilding one needs the registry [`Engine`] holds. Read an
/// audit back with [`Engine::deserialize_audit`].
///
/// # Schema
///
/// Generate under the **serialize** contract
/// ([`SchemaSettings::for_serialize`]). `reviews` and `usage` are
/// `skip_serializing_if`, and only that contract marks them
/// optional — `schema_for!` defaults to deserialize and declares
/// both required, so a generated client would reject responses this
/// crate really emits.
///
/// [`Engine`]: super::Engine
/// [`Engine::deserialize_audit`]: super::Engine::deserialize_audit
/// [`Report`]: elide::Report
/// [`SchemaSettings::for_serialize`]: schemars::generate::SchemaSettings::for_serialize
#[derive(Serialize, JsonSchema)]
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
    /// What the caller asserted when this document was analyzed:
    /// languages, jurisdictions, document tags.
    ///
    /// Carried back so [`Engine::anonymize`] compiles against the
    /// same vocabulary analyze used, and re-decodes under the same
    /// codec configuration, without the caller re-passing it.
    ///
    /// [`Engine::anonymize`]: super::Engine::anonymize
    pub context: DocumentContext,
    /// How this document was decoded when it was analyzed.
    ///
    /// Carried back so anonymize decodes identically: the entity
    /// offsets below are stored against the first decode, and a
    /// differently-rendered second one would not line up.
    pub codec: CodecParams,
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
