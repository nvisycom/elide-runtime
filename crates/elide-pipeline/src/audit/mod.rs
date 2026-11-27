//! Analyze → anonymize bridge: what [`Engine::analyze`] returns
//! and what [`Engine::anonymize`] accepts.
//!
//! Reviewer decisions land on the report itself: an added entity, a
//! corrected one, a suppressed one. They travel with the audit
//! because they are part of what analysis found, as amended.
//!
//! Hosts hold an [`Audit`] between the two passes and may persist
//! it however they like: serialize it directly, keep its
//! [`digest`](Audit::digest) where its carrier cannot change it, and
//! read it back against that digest with
//! [`Engine::deserialize_audit`].
//!
//! The enrichment content an analysis produced — an image's OCR
//! layout, an audio clip's transcript — is *not* on the audit. It
//! rides beside it in [`Analyzed`], so an audit stays references
//! and decisions while the content a re-run needs is persisted
//! (and governed) separately.
//!
//! [`Engine::analyze`]: crate::Engine::analyze
//! [`Engine::anonymize`]: crate::Engine::anonymize
//! [`Engine::deserialize_audit`]: crate::Engine::deserialize_audit

#[cfg(feature = "audit-csv")]
mod csv;
mod registered;

use elide::entity::audit::AuditHash;
use elide::modality::Modality;
use elide::{ArtifactSet, PartId, Report};
use elide_provider::{CodecParams, ComponentSelection, DocumentContext};
use elide_schema::recognition::Recognition;
use schemars::JsonSchema;
use serde::Serialize;
use uuid::Uuid;

pub use self::registered::{RegisteredComponents, RegisteredEnricher, RegisteredRecognizer};

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
/// [`Engine::analyze`]: crate::Engine::analyze
/// [`Engine::re_analyze`]: crate::Engine::re_analyze
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
/// ([`SchemaSettings::for_serialize`]). Fields that are
/// `skip_serializing_if` are optional only under that contract —
/// `schema_for!` defaults to deserialize and declares them
/// required, so a generated client would reject responses this
/// crate really emits.
///
/// [`Engine`]: crate::Engine
/// [`Engine::deserialize_audit`]: crate::Engine::deserialize_audit
/// [`Report`]: elide::Report
/// [`SchemaSettings::for_serialize`]: schemars::generate::SchemaSettings::for_serialize
#[derive(Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct Audit {
    /// The detections: elide's own report, every document and the
    /// parts nested in one, each entity carrying its provenance
    /// chain.
    ///
    /// Read it through [`Report::part`] and change it only through a
    /// review: [`EditSet::apply`] records each reviewer decision on
    /// the entities it touches.
    ///
    /// [`Report`]: elide::Report
    /// [`Report::part`]: elide::Report::part
    /// [`EditSet::apply`]: crate::entity::EditSet::apply
    pub report: Report,
    /// What the caller asserted when this document was analyzed:
    /// languages, jurisdictions, document tags.
    ///
    /// Carried back so [`Engine::anonymize`] compiles against the
    /// same vocabulary analyze used, and re-decodes under the same
    /// codec configuration, without the caller re-passing it.
    ///
    /// [`Engine::anonymize`]: crate::Engine::anonymize
    pub context: DocumentContext,
    /// The vocabularies this request introduced: labels elide does
    /// not ship, and how they were found.
    ///
    /// Carried back for the same reason [`context`] is. Anonymize
    /// compiles its catalog afresh, and a custom label missing
    /// from it resolves to nothing — the entity would be detected
    /// and then silently not redacted.
    ///
    /// [`context`]: Audit::context
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub recognition: Vec<Recognition>,
    /// How this document was decoded when it was analyzed.
    ///
    /// Carried back so anonymize decodes identically: the entity
    /// offsets below are stored against the first decode, and a
    /// differently-rendered second one would not line up.
    pub codec: CodecParams,
    /// Which recognizers this request selected.
    ///
    /// Carried back for the same reason [`codec`] is. Anonymize
    /// compiles its analyzers afresh, and a lineup narrower than
    /// the one analyze detected with would leave an entity found
    /// and then silently not redacted.
    ///
    /// [`codec`]: Audit::codec
    #[serde(default, skip_serializing_if = "ComponentSelection::is_empty")]
    pub selection: ComponentSelection,
}

impl Audit {
    /// A digest of the whole audit: the report, with every entity's
    /// trail, and what the request asserted about it.
    ///
    /// What [`Engine::deserialize_audit`] checks an audit against
    /// when it is read back. Keep it where the audit's carrier
    /// cannot change it: on the side that wrote the audit, in a
    /// write-once store, or signed with a key the deployment holds.
    /// The context, vocabularies, decode settings, and component
    /// selection are covered as well as the report, since anonymize
    /// acts on each.
    ///
    /// Changes when the report does, so take it again after
    /// applying a review.
    ///
    /// [`Engine::deserialize_audit`]: crate::Engine::deserialize_audit
    #[must_use]
    pub fn digest(&self) -> AuditHash {
        AuditHash::of_fields([
            self.report.digest().as_bytes().to_vec(),
            json(&self.context),
            json(&self.recognition),
            json(&self.codec),
            json(&self.selection),
        ])
    }

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
    /// Keyed on the absence of a [`Selected`] event rather than a
    /// redaction, so an operator that deliberately kept a value
    /// (`Keep`) does not read as a gap — it was chosen.
    ///
    /// Empty after a pass that covered everything it found. Worth
    /// checking before returning a document as de-identified.
    ///
    /// [`Selected`]: elide::entity::audit::AuditKind::Selected
    #[must_use]
    pub fn unhandled(&self) -> Vec<Unhandled> {
        let mut unhandled = Vec::new();
        elide_schema::each_modality!(|M| unhandled.extend(self.unhandled_in::<M>()));
        unhandled
    }

    /// One modality's unhandled detections, across the body and
    /// every container part.
    ///
    /// A nested part is where this matters most: a DOCX's embedded
    /// image is exactly the place a text-only policy leaves
    /// something behind, so reading the document alone would miss
    /// the case the method exists for.
    fn unhandled_in<M: Modality>(&self) -> impl Iterator<Item = Unhandled> + '_ {
        // The part tree alone: the sole document is itself a
        // depth-1 part, so reading it as well would report every
        // entity of a single-document report twice.
        let parts: Vec<PartId> = self.report.part_ids().map(|(id, _)| id.clone()).collect();
        parts
            .into_iter()
            .flat_map(move |id| {
                self.report
                    .part::<M>(&id)
                    .map(|part| part.entities())
                    .unwrap_or_default()
                    .iter()
            })
            .filter(|e| e.audit().selection().is_none() && !e.is_suppressed())
            .map(|e| Unhandled {
                entity_id: e.id(),
                modality: M::NAME,
                label: e.label().as_str().to_owned(),
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

/// `value` as JSON, for a digest.
///
/// Infallible for the request types an audit carries: plain data,
/// with no map keyed by anything but a string, and fields in
/// declaration order, so the same value always encodes the same.
fn json<T: Serialize>(value: &T) -> Vec<u8> {
    serde_json::to_vec(value).expect("an audit's request data serializes as JSON")
}
