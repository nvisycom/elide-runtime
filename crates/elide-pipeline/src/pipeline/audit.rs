//! Analyze → anonymize bridge: what [`Engine::analyze`] returns
//! and what [`Engine::anonymize`] accepts.
//!
//! Reviewer decisions land on the report itself: an added entity, a
//! corrected one, a suppressed one. They travel with the audit
//! because they are part of what analysis found, as amended.
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

use crate::entity::{Edit, EditBucket, EditSet};

/// What detection found in one document, plus what a reviewer
/// decided about it.
///
/// Wraps elide's [`Report`] with the three things elide does not
/// model: the recognition [`DocumentContext`] the entities were
/// scored against, how the document decoded, and the reviewer
/// decisions in [`edits`](Self::edits).
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
/// ([`SchemaSettings::for_serialize`]). `edits` and `usage` are
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
    ///
    /// [`Report`]: elide::Report
    /// [`include`]: elide::Report::include
    /// [`suppress`]: elide::Report::suppress
    /// [`entities`]: elide::Report::entities
    pub report: Report,
    /// What a reviewer changed: detections they added, corrected,
    /// or suppressed.
    ///
    /// A list rather than one decision per entity, because the
    /// operations feed independent channels — retagging an entity
    /// and suppressing it are both legitimate at once.
    #[serde(skip_serializing_if = "EditSet::is_empty")]
    pub edits: EditSet,
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
    /// Record a reviewer's edit.
    ///
    /// Appends rather than replaces: edits feed independent
    /// channels, so retagging an entity and suppressing it are both
    /// legitimate at once. Two edits that answer the same question
    /// differently are rejected by
    /// [`EditSet::validate`](crate::entity::EditSet::validate),
    /// which [`Engine::anonymize`] runs before applying anything.
    ///
    /// The modality is the entity's own, so an edit carrying a
    /// location carries that modality's — a text entity cannot be
    /// given an image span, and the mismatch will not compile.
    ///
    /// [`Engine::anonymize`]: super::Engine::anonymize
    pub fn edit<M: EditBucket>(&mut self, edit: Edit<M>) -> &mut Self {
        M::bucket_mut(&mut self.edits).push(edit);
        self
    }

    /// Every edit recorded for the entity `id`, in order.
    #[must_use]
    pub fn edits_for<M: EditBucket>(&self, id: Uuid) -> Vec<&Edit<M>> {
        M::bucket(&self.edits)
            .iter()
            .filter(|edit| edit.target() == Some(id))
            .collect()
    }

    /// Drop every edit recorded for the entity `id`, restoring it to
    /// whatever the policy set picks.
    ///
    /// Returns how many were dropped.
    pub fn unedit<M: EditBucket>(&mut self, id: Uuid) -> usize {
        let bucket = M::bucket_mut(&mut self.edits);
        let before = bucket.len();
        bucket.retain(|edit| edit.target() != Some(id));
        before - bucket.len()
    }

    /// Whether the entity `id` will be left alone.
    ///
    /// A pending [`Edit::Suppress`] wins over the entity's trail.
    /// With no pending edit this falls back to the trail, so an
    /// applied suppression still reads as suppressed after a round
    /// trip.
    #[must_use]
    pub fn is_suppressed<M: EditBucket + 'static>(&self, id: Uuid) -> bool {
        // Only the outcome channel speaks to this; a retag says
        // nothing about whether the entity is redacted. Read from
        // the last edit backwards, so a reviewer's latest word
        // wins over an earlier one they replaced.
        let pending = self
            .edits_for::<M>(id)
            .into_iter()
            .rev()
            .find_map(|edit| match edit {
                Edit::Suppress { .. } => Some(true),
                Edit::Add { .. } | Edit::Retag { .. } => None,
            });

        pending.unwrap_or_else(|| {
            self.report
                .entities::<M>()
                .and_then(|entities| entities.iter().find(|e| e.id == id))
                .is_some_and(elide::entity::Entity::is_suppressed)
        })
    }
}
