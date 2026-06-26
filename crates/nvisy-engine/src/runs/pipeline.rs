//! Per-document analyze + apply pipelines.
//!
//! - [`analyze_document`] decodes bytes, resolves modality,
//!   compiles the per-modality analyzer from the
//!   [`AnalyzerParams`], recognizes entities, and wraps them in a
//!   [`DocBody`].
//! - [`apply_document`] decodes bytes again, layers reviewer
//!   overrides on top of the policy-driven anonymizer, applies
//!   it to the persisted entities, and returns the redacted
//!   bytes.
//!
//! Pure functions over the elide toolkit; no fjall I/O. The
//! orchestrator in [`super::orchestrate`] drives them as a
//! bounded-concurrency stream and persists the resulting body /
//! artifact.

use bytes::Bytes;
use elide::codec::{DocumentHandle, FormatRegistry};
use elide::detection::Analyzer;
use elide::redaction::Anonymizer;
use elide_core::entity::Entity;
use elide_core::modality::Modality;
use elide_core::modality::audio::Audio;
use elide_core::modality::image::Image;
use elide_core::modality::tabular::Tabular;
use elide_core::modality::text::Text;
use elide_core::recognition::Scope;
use nvisy_core::plan::AnalyzerParams;
use nvisy_core::policy::Policy;
use nvisy_core::{Error, Result};

use super::filter::{DocumentFacts, policy_applies};
use super::state::{DocBody, EntityRecord, ModalityKind};
use crate::analyzer::{build_catalog, compile_audio, compile_image, compile_tabular, compile_text};
use crate::anonymizer::{
    attach_override_audio, attach_override_image, attach_override_tabular, attach_override_text,
    attach_policies_audio, attach_policies_image, attach_policies_tabular, attach_policies_text,
};

const COMPONENT: &str = "runs::pipeline";

/// Outcome of analyzing one document end-to-end.
pub(super) struct AnalyzeOutcome {
    /// Modality elide's codec resolved the bytes to.
    pub modality: ModalityKind,
    /// Recognized entities, wrapped in [`EntityRecord`] for
    /// persistence (no overrides set yet — those flow through the
    /// reviewer surface).
    pub body: DocBody,
}

/// Decode `bytes` via `registry`, resolve the modality, compile
/// the per-modality analyzer from `spec`, recognize entities,
/// return them in the modality-specific [`DocBody`] variant.
///
/// `extension` is the codec discriminator (case-insensitive, no
/// leading dot) — e.g. `"txt"`, `"csv"`, `"png"`, `"wav"`.
pub(super) async fn analyze_document(
    registry: &FormatRegistry,
    bytes: Bytes,
    extension: &str,
    spec: &AnalyzerParams,
) -> Result<AnalyzeOutcome> {
    let handle = registry.decode(bytes, extension).await.map_err(|err| {
        Error::validation(
            format!("codec decode failed for extension {extension:?}"),
            COMPONENT,
        )
        .with_source(err)
    })?;

    if handle.is::<Text>() {
        let typed = handle
            .into::<Text>()
            .map_err(|_| modality_mismatch_err("Text"))?;
        let (analyzer, scope) = compile_text(spec).map_err(compile_err)?;
        let entities = recognize::<Text>(analyzer, scope, typed).await?;
        let body = DocBody::Text {
            entities: entities.into_iter().map(EntityRecord::new).collect(),
        };
        return Ok(AnalyzeOutcome {
            modality: ModalityKind::Text,
            body,
        });
    }

    if handle.is::<Tabular>() {
        let typed = handle
            .into::<Tabular>()
            .map_err(|_| modality_mismatch_err("Tabular"))?;
        let (analyzer, scope) = compile_tabular(spec).map_err(compile_err)?;
        let entities = recognize::<Tabular>(analyzer, scope, typed).await?;
        let body = DocBody::Tabular {
            entities: entities.into_iter().map(EntityRecord::new).collect(),
        };
        return Ok(AnalyzeOutcome {
            modality: ModalityKind::Tabular,
            body,
        });
    }

    if handle.is::<Image>() {
        let typed = handle
            .into::<Image>()
            .map_err(|_| modality_mismatch_err("Image"))?;
        let (analyzer, scope) = compile_image(spec).map_err(compile_err)?;
        let entities = recognize::<Image>(analyzer, scope, typed).await?;
        let body = DocBody::Image {
            entities: entities.into_iter().map(EntityRecord::new).collect(),
        };
        return Ok(AnalyzeOutcome {
            modality: ModalityKind::Image,
            body,
        });
    }

    if handle.is::<Audio>() {
        let typed = handle
            .into::<Audio>()
            .map_err(|_| modality_mismatch_err("Audio"))?;
        let (analyzer, scope) = compile_audio(spec).map_err(compile_err)?;
        let entities = recognize::<Audio>(analyzer, scope, typed).await?;
        let body = DocBody::Audio {
            entities: entities.into_iter().map(EntityRecord::new).collect(),
        };
        return Ok(AnalyzeOutcome {
            modality: ModalityKind::Audio,
            body,
        });
    }

    Err(Error::validation(
        format!("codec resolved {extension:?} to an unsupported modality"),
        COMPONENT,
    ))
}

async fn recognize<M: Modality>(
    analyzer: Analyzer<M>,
    scope: Scope<M>,
    mut handle: DocumentHandle<M>,
) -> Result<Vec<Entity<M>>> {
    analyzer
        .analyze_stream(&mut handle, &scope)
        .await
        .map_err(|err| Error::internal("analyzer fanout failed", COMPONENT).with_source(err))
}

fn modality_mismatch_err(expected: &'static str) -> Error {
    Error::internal(
        format!(
            "codec advertised modality {expected} but downcast failed — \
             elide handle::is/into mismatch"
        ),
        COMPONENT,
    )
}

/// Translate an `elide::Error` from a `compile_*` call into the
/// runtime's error type. Compile failures are caller-driven
/// (e.g. unsupported recognizer for the modality), so they map
/// to `Validation`.
fn compile_err(err: elide::Error) -> Error {
    Error::validation(format!("analyzer compile failed: {err}"), COMPONENT).with_source(err)
}

/// Outcome of applying redactions to one document.
pub(super) struct ApplyOutcome {
    /// Encoded bytes of the redacted document, ready to persist
    /// in the `run_artifacts` keyspace.
    pub bytes: Bytes,
}

/// Re-decode `bytes`, layer reviewer overrides + applicable
/// policies into a per-modality anonymizer, apply it, re-encode,
/// return the redacted bytes.
///
/// `entities` is the persisted recognition output (one
/// [`EntityRecord`] per entity, possibly carrying a reviewer
/// override). `policies` is the full resolved policy set; the
/// pipeline filters to those whose [`Policy::applies_when`]
/// holds against `facts` (the merged descriptor + per-request
/// metadata). `spec` is the same [`AnalyzerParams`] that drove
/// analyze — needed for the label catalog.
pub(super) async fn apply_document(
    registry: &FormatRegistry,
    bytes: Bytes,
    extension: &str,
    spec: &AnalyzerParams,
    policies: &[Policy],
    facts: &DocumentFacts<'_>,
    body: &DocBody,
) -> Result<ApplyOutcome> {
    let handle = registry.decode(bytes, extension).await.map_err(|err| {
        Error::validation(
            format!("codec decode failed for extension {extension:?}"),
            COMPONENT,
        )
        .with_source(err)
    })?;

    let scoped = || policies.iter().filter(|p| policy_applies(p, facts));

    match body {
        DocBody::Text { entities } => {
            let typed = handle
                .into::<Text>()
                .map_err(|_| modality_mismatch_err("Text"))?;
            let anonymizer = build_text_anonymizer(spec, scoped(), entities)?;
            let bytes = run_anonymize::<Text>(anonymizer, typed, entities).await?;
            Ok(ApplyOutcome { bytes })
        }
        DocBody::Tabular { entities } => {
            let typed = handle
                .into::<Tabular>()
                .map_err(|_| modality_mismatch_err("Tabular"))?;
            let anonymizer = build_tabular_anonymizer(spec, scoped(), entities)?;
            let bytes = run_anonymize::<Tabular>(anonymizer, typed, entities).await?;
            Ok(ApplyOutcome { bytes })
        }
        DocBody::Image { entities } => {
            let typed = handle
                .into::<Image>()
                .map_err(|_| modality_mismatch_err("Image"))?;
            let anonymizer = build_image_anonymizer(spec, scoped(), entities);
            let bytes = run_anonymize::<Image>(anonymizer, typed, entities).await?;
            Ok(ApplyOutcome { bytes })
        }
        DocBody::Audio { entities } => {
            let typed = handle
                .into::<Audio>()
                .map_err(|_| modality_mismatch_err("Audio"))?;
            let anonymizer = build_audio_anonymizer(spec, scoped(), entities);
            let bytes = run_anonymize::<Audio>(anonymizer, typed, entities).await?;
            Ok(ApplyOutcome { bytes })
        }
    }
}

fn build_text_anonymizer<'a>(
    spec: &AnalyzerParams,
    policies: impl Iterator<Item = &'a Policy>,
    entities: &[EntityRecord<Text>],
) -> Result<Anonymizer<Text>> {
    let catalog = build_catalog(spec);
    let mut anonymizer = Anonymizer::<Text>::new().with_catalog(catalog);
    for record in entities {
        if let Some(action) = &record.r#override {
            anonymizer =
                attach_override_text(anonymizer, record.entity.id, action).map_err(compile_err)?;
        }
    }
    attach_policies_text(anonymizer, policies).map_err(compile_err)
}

fn build_tabular_anonymizer<'a>(
    spec: &AnalyzerParams,
    policies: impl Iterator<Item = &'a Policy>,
    entities: &[EntityRecord<Tabular>],
) -> Result<Anonymizer<Tabular>> {
    let catalog = build_catalog(spec);
    let mut anonymizer = Anonymizer::<Tabular>::new().with_catalog(catalog);
    for record in entities {
        if let Some(action) = &record.r#override {
            anonymizer = attach_override_tabular(anonymizer, record.entity.id, action)
                .map_err(compile_err)?;
        }
    }
    attach_policies_tabular(anonymizer, policies).map_err(compile_err)
}

fn build_image_anonymizer<'a>(
    spec: &AnalyzerParams,
    policies: impl Iterator<Item = &'a Policy>,
    entities: &[EntityRecord<Image>],
) -> Anonymizer<Image> {
    let catalog = build_catalog(spec);
    let mut anonymizer = Anonymizer::<Image>::new().with_catalog(catalog);
    for record in entities {
        if let Some(action) = &record.r#override {
            anonymizer = attach_override_image(anonymizer, record.entity.id, action);
        }
    }
    attach_policies_image(anonymizer, policies)
}

fn build_audio_anonymizer<'a>(
    spec: &AnalyzerParams,
    policies: impl Iterator<Item = &'a Policy>,
    entities: &[EntityRecord<Audio>],
) -> Anonymizer<Audio> {
    let catalog = build_catalog(spec);
    let mut anonymizer = Anonymizer::<Audio>::new().with_catalog(catalog);
    for record in entities {
        if let Some(action) = &record.r#override {
            anonymizer = attach_override_audio(anonymizer, record.entity.id, action);
        }
    }
    attach_policies_audio(anonymizer, policies)
}

async fn run_anonymize<M>(
    anonymizer: Anonymizer<M>,
    mut handle: DocumentHandle<M>,
    entities: &[EntityRecord<M>],
) -> Result<Bytes>
where
    M: Modality + Clone,
{
    // elide's `anonymize` takes `&mut [Entity<M>]` and updates
    // provenance in place; we clone the entities out of the
    // persisted records since the persisted body is the
    // source of truth for the next call (idempotent apply).
    let mut working: Vec<Entity<M>> = entities.iter().map(|r| r.entity.clone()).collect();
    anonymizer
        .anonymize(&mut handle, &mut working)
        .await
        .map_err(|err| Error::internal("anonymize failed", COMPONENT).with_source(err))?;

    let content = handle.encode().map_err(|err| {
        Error::internal("post-anonymize encode failed", COMPONENT).with_source(err)
    })?;
    Ok(content.into_bytes())
}
