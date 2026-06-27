//! Per-document analyze + apply over the engine's [`Orchestrator`].
//!
//! - [`analyze_document`] decodes bytes, hands the
//!   [`UntypedDocumentHandle`] to [`Engine::analyze`], and projects
//!   the returned [`elide::Report`] onto the persisted [`DocBody`]
//!   shape (body entities only — container parts are detected by
//!   the orchestrator but not yet captured in the persisted body;
//!   a future schema bump will retain them).
//! - [`apply_document`] decodes bytes again, rebuilds an
//!   [`elide::Report`] from the persisted body entities, and
//!   delegates to [`Engine::anonymize_with`] with the reviewer
//!   overrides extracted from the body's records. The encoded
//!   bytes are read back from the document handle and returned.
//!
//! Pure functions over [`Engine`]; no fjall I/O. The run
//! orchestrator in [`super::orchestrate`] drives them as a
//! bounded-concurrency stream and persists their inputs/outputs.
//!
//! [`Engine`]: crate::Engine
//! [`Engine::analyze`]: crate::Engine::analyze
//! [`Engine::anonymize_with`]: crate::Engine::anonymize_with
//! [`Orchestrator`]: elide::Orchestrator
//! [`UntypedDocumentHandle`]: elide::codec::UntypedDocumentHandle

use std::mem;

use bytes::Bytes;
use elide::codec::UntypedDocumentHandle;
use elide_core::entity::Entity;
use elide_core::modality::Modality;
use elide_core::modality::audio::Audio;
use elide_core::modality::image::Image;
use elide_core::modality::tabular::Tabular;
use elide_core::modality::text::Text;
use nvisy_core::plan::AnalyzerParams;
use nvisy_core::policy::{Policy, RuleAction};
use nvisy_core::{Error, Result};
use uuid::Uuid;

use super::filter::{DocumentFacts, policy_applies};
use super::state::{DocBody, EntityRecord, ModalityKind};
use crate::Engine;

const COMPONENT: &str = "runs::pipeline";

/// Outcome of analyzing one document end-to-end.
pub(super) struct AnalyzeOutcome {
    /// Modality elide's codec resolved the bytes to.
    pub modality: ModalityKind,
    /// Recognized body entities, wrapped in [`EntityRecord`] for
    /// persistence (no overrides set yet — those flow through the
    /// reviewer surface).
    pub body: DocBody,
}

/// Decode `bytes`, drive [`Engine::analyze`], project the body
/// entities of the returned [`elide::Report`] onto the persisted
/// [`DocBody`].
///
/// Container-part entities (PDF embedded images, archive members,
/// …) are detected by the orchestrator but discarded at the
/// persistence boundary today; following slices will evolve
/// [`DocBody`] to retain them.
///
/// `extension` is the codec discriminator (case-insensitive, no
/// leading dot) — e.g. `"txt"`, `"csv"`, `"png"`, `"wav"`.
pub(super) async fn analyze_document(
    engine: &Engine,
    bytes: Bytes,
    extension: &str,
    spec: &AnalyzerParams,
) -> Result<AnalyzeOutcome> {
    let mut handle = decode(engine, bytes, extension).await?;
    let mut report = engine.analyze(&mut handle, spec).await?;

    // The orchestrator decided which pipeline matched the body
    // (one of Text / Tabular / Image / Audio) — recover that by
    // peeking at the report's body entity slot for each modality.
    if let Some(entities) = report.entities::<Text>().map(mem::take) {
        return Ok(AnalyzeOutcome {
            modality: ModalityKind::Text,
            body: DocBody::Text {
                entities: entities.into_iter().map(EntityRecord::new).collect(),
            },
        });
    }
    if let Some(entities) = report.entities::<Tabular>().map(mem::take) {
        return Ok(AnalyzeOutcome {
            modality: ModalityKind::Tabular,
            body: DocBody::Tabular {
                entities: entities.into_iter().map(EntityRecord::new).collect(),
            },
        });
    }
    if let Some(entities) = report.entities::<Image>().map(mem::take) {
        return Ok(AnalyzeOutcome {
            modality: ModalityKind::Image,
            body: DocBody::Image {
                entities: entities.into_iter().map(EntityRecord::new).collect(),
            },
        });
    }
    if let Some(entities) = report.entities::<Audio>().map(mem::take) {
        return Ok(AnalyzeOutcome {
            modality: ModalityKind::Audio,
            body: DocBody::Audio {
                entities: entities.into_iter().map(EntityRecord::new).collect(),
            },
        });
    }

    Err(Error::validation(
        format!(
            "codec resolved {extension:?} to a modality the orchestrator \
             has no pipeline for"
        ),
        COMPONENT,
    ))
}

/// Outcome of applying redactions to one document.
pub(super) struct ApplyOutcome {
    /// Encoded bytes of the redacted document, ready to persist
    /// via the [`FileRegistry`] as a new output file.
    ///
    /// [`FileRegistry`]: crate::FileRegistry
    pub bytes: Bytes,
}

/// Re-decode `bytes`, rebuild an [`elide::Report`] from the
/// persisted body entities, drive [`Engine::anonymize_with`] with
/// the reviewer overrides extracted from the body's records, and
/// return the re-encoded redacted bytes.
///
/// `policies` is the full resolved policy set; pre-filtered to
/// those whose [`Policy::applies_when`] holds against `facts`.
/// `spec` is the same [`AnalyzerParams`] that drove analyze —
/// needed for the label catalog. `body` is the persisted
/// recognition output for this doc; reviewer overrides ride on
/// its [`EntityRecord`]s.
pub(super) async fn apply_document(
    engine: &Engine,
    bytes: Bytes,
    extension: &str,
    spec: &AnalyzerParams,
    policies: &[Policy],
    facts: &DocumentFacts<'_>,
    body: &DocBody,
) -> Result<ApplyOutcome> {
    let mut handle = decode(engine, bytes, extension).await?;
    let scoped: Vec<Policy> = policies
        .iter()
        .filter(|p| policy_applies(p, facts))
        .cloned()
        .collect();

    let (modality, report, overrides) = match body {
        DocBody::Text { entities } => (
            ModalityKind::Text,
            build_report::<Text>(entities),
            collect_overrides(entities),
        ),
        DocBody::Tabular { entities } => (
            ModalityKind::Tabular,
            build_report::<Tabular>(entities),
            collect_overrides(entities),
        ),
        DocBody::Image { entities } => (
            ModalityKind::Image,
            build_report::<Image>(entities),
            collect_overrides(entities),
        ),
        DocBody::Audio { entities } => (
            ModalityKind::Audio,
            build_report::<Audio>(entities),
            collect_overrides(entities),
        ),
    };

    engine
        .anonymize_with(&mut handle, spec, &scoped, modality, &overrides, report)
        .await?;

    encode_redacted(handle, modality)
}

/// Rebuild a body-only [`elide::Report`] from the persisted
/// per-entity records. The entities are cloned out (the persisted
/// body is the source of truth for re-apply idempotency).
fn build_report<M>(records: &[EntityRecord<M>]) -> elide::Report
where
    M: Modality + 'static,
    Vec<Entity<M>>: elide::EntityGroup,
    Entity<M>: Clone,
{
    let entities: Vec<Entity<M>> = records.iter().map(|r| r.entity.clone()).collect();
    elide::Report::new().insert_body::<M>(entities)
}

fn collect_overrides<M: Modality>(records: &[EntityRecord<M>]) -> Vec<(Uuid, RuleAction)> {
    records
        .iter()
        .filter_map(|r| r.r#override.as_ref().map(|a| (r.entity.id, a.clone())))
        .collect()
}

/// After `anonymize_with` mutated `handle` in place, recover the
/// typed handle for the doc's body modality and re-encode it.
/// `handle` was a typed `DocumentHandle<M>` before being erased;
/// the apply-time re-encode needs the typed form because
/// [`elide::codec::DocumentHandle::encode`] is per-modality.
fn encode_redacted(handle: UntypedDocumentHandle, modality: ModalityKind) -> Result<ApplyOutcome> {
    match modality {
        ModalityKind::Text => encode_typed::<Text>(handle, "Text"),
        ModalityKind::Tabular => encode_typed::<Tabular>(handle, "Tabular"),
        ModalityKind::Image => encode_typed::<Image>(handle, "Image"),
        ModalityKind::Audio => encode_typed::<Audio>(handle, "Audio"),
    }
}

fn encode_typed<M>(handle: UntypedDocumentHandle, name: &'static str) -> Result<ApplyOutcome>
where
    M: Modality,
{
    let typed = handle.into::<M>().map_err(|_| {
        Error::internal(
            format!(
                "post-apply re-encode: handle is not {name} — orchestrator \
                 returned a handle of a different modality than analyze \
                 recorded"
            ),
            COMPONENT,
        )
    })?;
    let content = typed
        .encode()
        .map_err(|err| Error::internal("post-apply encode failed", COMPONENT).with_source(err))?;
    Ok(ApplyOutcome {
        bytes: content.into_bytes(),
    })
}

async fn decode(engine: &Engine, bytes: Bytes, extension: &str) -> Result<UntypedDocumentHandle> {
    engine
        .formats()
        .decode(bytes, extension)
        .await
        .map_err(|err| {
            Error::validation(
                format!("codec decode failed for extension {extension:?}"),
                COMPONENT,
            )
            .with_source(err)
        })
}
