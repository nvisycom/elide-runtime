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
//! The enrichment content an analysis produced — an image's OCR
//! layout, an audio clip's transcript — is *not* on the audit. It
//! rides beside it in [`Analyzed`], so an audit stays references
//! and decisions while the content a re-run needs is persisted
//! (and governed) separately.
//!
//! [`Engine::analyze`]: super::Engine::analyze
//! [`Engine::anonymize`]: super::Engine::anonymize
//! [`Engine::deserialize_audit`]: super::Engine::deserialize_audit

use elide::codec::PartId;
use elide::modality::Modality;
use elide::modality::audio::Audio;
use elide::modality::image::Image;
use elide::modality::tabular::Tabular;
use elide::modality::text::Text;
use elide::recognition::UsageReport;
use elide::{ArtifactSet, Report};
use elide_provider::{CodecParams, DocumentContext};
use schemars::JsonSchema;
use serde::Serialize;
use uuid::Uuid;

/// What [`Engine::analyze`] produced: the [`Audit`] and, beside
/// it, the enrichment content the pass extracted.
///
/// The two are separate because they are governed differently. An
/// [`Audit`] is references and decisions — entity locations plus
/// their provenance — and carries no content of its own, so it is
/// safe to hand around, log, and export. An [`ArtifactSet`] is the
/// opposite: an OCR layout or a transcript *is* the source text,
/// so a recording's transcript is as sensitive as the recording.
/// Keeping it out of the audit means serializing an audit can
/// never leak the document's content by accident; a caller that
/// wants the artifacts persists them deliberately.
///
/// Persist both to re-detect after a review gap without paying for
/// OCR/STT twice — see [`Engine::re_analyze`].
///
/// [`Engine::analyze`]: super::Engine::analyze
/// [`Engine::re_analyze`]: super::Engine::re_analyze
pub struct Analyzed {
    /// What detection found, and what the policies would do about
    /// it: the reviewable half.
    pub audit: Audit,
    /// The enrichment the pass extracted — an image's OCR layout,
    /// an audio clip's transcript. Empty for a document whose
    /// modality needs no enrichment (text, tabular).
    ///
    /// Carries document content: govern it as you would the
    /// document itself.
    pub artifacts: ArtifactSet,
}

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
    /// Entities no policy acted on, by modality and label.
    ///
    /// An entity here was detected, was not suppressed by a
    /// reviewer, and no operator was even *picked* for it — so it
    /// survives into the output with nothing recording why. The
    /// usual cause is a policy whose rules name an operator for one
    /// modality and not the entity's: the rule matches, attaches
    /// nothing, and the value passes through unredacted.
    ///
    /// Keyed on the absence of a [`Selection`] rather than a
    /// redaction, so an operator that deliberately kept a value
    /// (`Keep`) does not read as a gap — it was chosen.
    ///
    /// Empty after a pass that covered everything it found. Worth
    /// checking before returning a document as de-identified.
    ///
    /// [`Selection`]: elide::entity::audit::AuditKind::Selection
    #[must_use]
    pub fn unhandled(&self) -> Vec<Unhandled> {
        self.unhandled_in::<Text>()
            .chain(self.unhandled_in::<Tabular>())
            .chain(self.unhandled_in::<Image>())
            .chain(self.unhandled_in::<Audio>())
            .collect()
    }

    /// One modality's unhandled detections, across the body and
    /// every container part.
    ///
    /// A part is where this matters most: a DOCX's embedded image
    /// is exactly the place a text-only policy leaves something
    /// behind, so reading the body alone would miss the case the
    /// method exists for.
    fn unhandled_in<M: Modality>(&self) -> impl Iterator<Item = Unhandled> + '_ {
        let parts: Vec<PartId> = self.report.part_ids().map(|(id, _)| id.clone()).collect();
        let body = self.report.entities::<M>().unwrap_or_default();
        let in_parts = parts.into_iter().flat_map(move |id| {
            self.report
                .part_entities::<M>(&id)
                .unwrap_or_default()
                .iter()
        });

        body.iter()
            .chain(in_parts)
            .filter(|e| e.audit.selection().is_none() && !e.audit.is_suppressed())
            .map(|e| Unhandled {
                entity_id: e.id,
                modality: M::NAME,
                label: e.label.as_str().to_owned(),
            })
    }

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

/// One detection no policy acted on. See
/// [`Audit::unhandled`](Audit::unhandled).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Unhandled {
    /// The entity that survived.
    pub entity_id: Uuid,
    /// The modality it belongs to.
    pub modality: &'static str,
    /// What it was detected as.
    pub label: String,
}
