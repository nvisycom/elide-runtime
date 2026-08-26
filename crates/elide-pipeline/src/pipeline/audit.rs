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
use elide::modality::Modality;
use elide::recognition::UsageReport;
use elide_provider::{CodecParams, DocumentContext};
use schemars::JsonSchema;
use serde::Serialize;
use uuid::Uuid;

/// What detection found in one document.
///
/// Wraps elide's [`Report`] with the things elide does not model:
/// the recognition [`DocumentContext`] the entities were scored
/// against, how the document decoded, and what the pass cost.
///
/// Reviewer edits are not here. They are the caller's own input,
/// applied to the report before anonymize
/// ([`EditSet::apply`](crate::entity::EditSet::apply)), so an audit
/// carries what analysis found *as amended* rather than the
/// amendments themselves.
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
/// ([`SchemaSettings::for_serialize`]). `usage` is
/// `skip_serializing_if`, and only that contract marks it optional
/// — `schema_for!` defaults to deserialize and declares it
/// required, so a generated client would reject responses this
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
    /// Whether the entity `id` will be left alone.
    ///
    /// Reads the entity's own trail, so an applied suppression
    /// still reports as suppressed after a round trip.
    ///
    /// Pending edits are not consulted: they are a separate input
    /// the caller holds, and whether one *would* suppress this
    /// entity is that set's question rather than the audit's.
    #[must_use]
    pub fn is_suppressed<M: Modality>(&self, id: Uuid) -> bool {
        self.report
            .entity_anywhere::<M>(id)
            .is_some_and(elide::entity::Entity::is_suppressed)
    }
}
