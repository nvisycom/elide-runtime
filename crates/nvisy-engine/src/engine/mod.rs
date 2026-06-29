//! [`Engine`]: the runtime entry point — long-lived state plus the
//! verbs that drive a per-request [`Orchestrator`].
//!
//! Two pieces of long-lived state:
//!
//! - The [`RegistryHandle`] over [`fjall`] (policies, contexts,
//!   files, runs). Multi-tenant, actor-scoped.
//! - The [`FormatRegistry`] over elide's codec set. Decodes raw
//!   bytes into a modality-typed [`DocumentHandle`] at analyze +
//!   apply time.
//!
//! Both fields are `Arc`-backed under the hood, so [`Engine`]
//! clones cheaply; one is opened at server start and a clone goes
//! to every HTTP handler.
//!
//! ## Per-document verbs
//!
//! - [`Engine::analyze_document`] decodes raw bytes, builds an
//!   [`Orchestrator`] with one pipeline per modality + the request
//!   scope, runs detection, and projects the body entities onto
//!   the persistence-shaped [`AnalyzeOutcome`].
//! - [`Engine::apply_document`] decodes raw bytes again, rebuilds
//!   a body-only [`Report`] from the persisted entities, layers
//!   the reviewer overrides + filtered policy set onto the
//!   modality's anonymizer, drives redaction, and returns the
//!   re-encoded bytes via [`ApplyOutcome`].
//!
//! Both methods build a fresh [`Orchestrator`] per call — it is a
//! small map of trait objects keyed by modality `TypeId`, cheap
//! to construct. The per-call shape lets us re-resolve policies +
//! scope per document at apply time without mutating a shared
//! anonymizer.
//!
//! ## Run lifecycle
//!
//! Free functions in [`super::runs`] (`start`, `apply`, `get`,
//! `list`, `cancel`, `delete`, `override_entity`) drive the
//! multi-doc batched run lifecycle, fanning the per-doc verbs
//! above out under a concurrency cap with per-doc timeouts.
//!
//! [`FormatRegistry`]: elide::codec::FormatRegistry
//! [`DocumentHandle`]: elide::codec::DocumentHandle
//! [`Orchestrator`]: elide::Orchestrator
//! [`Report`]: elide::Report
//! [`fjall`]: ::fjall

pub(crate) mod analyzer;
pub(crate) mod anonymizer;

use std::any::TypeId;
use std::collections::HashMap;
use std::mem;
use std::path::Path;
use std::sync::Arc;

use bytes::Bytes;
use elide::Orchestrator;
use elide::codec::{FormatRegistry, PartId, UntypedDocumentHandle};
use elide::redaction::Anonymizer;
use elide_core::entity::Entity;
use elide_core::modality::Modality;
use elide_core::modality::audio::Audio;
use elide_core::modality::image::Image;
use elide_core::modality::tabular::Tabular;
use elide_core::modality::text::Text;
use nvisy_core::RawDocument;
use nvisy_core::plan::AnalyzerParams;
use nvisy_core::policy::{Policy, RuleAction};
use nvisy_core::{Error, Result};
use uuid::Uuid;

use self::analyzer::{AnalyzerCompile, LabelCatalogCompile};
use self::anonymizer::{
    attach_override_audio, attach_override_image, attach_override_tabular, attach_override_text,
    attach_policies_audio, attach_policies_image, attach_policies_tabular, attach_policies_text,
};
use crate::registry::RegistryHandle;
use crate::runs::{DocBody, EntityGroup, EntityRecord};

const COMPONENT: &str = "engine";

/// Cheaply-cloneable runtime adapter: persistence + codecs + the
/// per-request orchestrator constructor.
#[derive(Clone)]
pub struct Engine {
    registry: RegistryHandle,
    formats: Arc<FormatRegistry>,
}

/// Outcome of applying redactions to one document.
pub struct ApplyOutcome {
    /// Encoded bytes of the redacted document, ready to persist
    /// via the [`FileRegistry`] as a new output file.
    ///
    /// [`FileRegistry`]: crate::FileRegistry
    pub bytes: Bytes,
}

impl Engine {
    /// Open (or create) the engine database at `path` and pair it
    /// with elide's built-in codec set
    /// ([`FormatRegistry::with_builtin`]).
    pub fn open(path: &Path) -> Result<Self> {
        let registry = RegistryHandle::open(path)?;
        let formats = Arc::new(FormatRegistry::with_builtin());
        Ok(Self { registry, formats })
    }

    /// Open (or create) the engine database at `path` and pair it
    /// with a caller-supplied `formats` registry. Useful for tests
    /// that need to register fake codecs, or for deployments that
    /// extend the built-in set.
    pub fn with_formats(path: &Path, formats: FormatRegistry) -> Result<Self> {
        let registry = RegistryHandle::open(path)?;
        Ok(Self {
            registry,
            formats: Arc::new(formats),
        })
    }

    /// The persistence registry. Holds the fjall keyspaces every
    /// resource module reads and writes.
    pub fn registry(&self) -> &RegistryHandle {
        &self.registry
    }

    /// The codec registry. Pipeline calls reach for it to decode
    /// raw bytes into an [`UntypedDocumentHandle`].
    pub fn formats(&self) -> &FormatRegistry {
        &self.formats
    }

    /// Flush pending writes to disk. The server's HTTP layer
    /// calls this on graceful shutdown.
    pub fn sync(&self) -> Result<()> {
        self.registry.sync()
    }

    /// Decode `document`, drive [`Orchestrator::analyze`], project
    /// the report onto the persistence-shaped [`DocBody`].
    ///
    /// Captures the body group *and* every container part group
    /// (DOCX embedded images, archive members, …) the orchestrator
    /// returned; each persisted group carries its own modality
    /// tag via its [`EntityGroup`] variant.
    ///
    /// `correlation_id` is server-minted (typically the run id) —
    /// the orchestrator threads it into tracing spans on the
    /// detection path.
    ///
    /// [`Orchestrator::analyze`]: elide::Orchestrator::analyze
    pub async fn analyze_document(
        &self,
        document: RawDocument,
        spec: &AnalyzerParams,
        correlation_id: Uuid,
    ) -> Result<DocBody> {
        let extension = document.extension.clone();
        let mut handle = self.decode(document).await?;
        let orchestrator = self.build_orchestrator(spec, &[], &[], correlation_id)?;
        let mut report = orchestrator.analyze(&mut handle).await.map_err(|err| {
            Error::internal("orchestrator analyze failed", COMPONENT).with_source(err)
        })?;

        // Walk the body modality slots in order; the first that
        // returns Some is the body modality the orchestrator's
        // codec resolved. `body` ends up None only if no pipeline
        // accepted the body — defensive, since all four are wired.
        let body_group = take_body_text(&mut report)
            .or_else(|| take_body_tabular(&mut report))
            .or_else(|| take_body_image(&mut report))
            .or_else(|| take_body_audio(&mut report))
            .ok_or_else(|| {
                Error::validation(
                    format!(
                        "codec resolved {extension:?} to a modality the orchestrator \
                         has no pipeline for"
                    ),
                    COMPONENT,
                )
            })?;

        // Walk the parts in the same way: collect (id, modality)
        // pairs first (read-borrow), then drain each part with the
        // matching typed accessor (write-borrow).
        let part_ids: Vec<(PartId, TypeId)> = report
            .part_ids()
            .map(|(id, type_id)| (id.clone(), type_id))
            .collect();
        let mut parts = HashMap::with_capacity(part_ids.len());
        for (id, type_id) in part_ids {
            let group = if type_id == TypeId::of::<Text>() {
                take_part_text(&mut report, &id)
            } else if type_id == TypeId::of::<Tabular>() {
                take_part_tabular(&mut report, &id)
            } else if type_id == TypeId::of::<Image>() {
                take_part_image(&mut report, &id)
            } else if type_id == TypeId::of::<Audio>() {
                take_part_audio(&mut report, &id)
            } else {
                // Pipeline modality the engine doesn't model. Skip.
                continue;
            };
            if let Some(group) = group {
                parts.insert(id.as_str().to_owned(), group);
            }
        }

        Ok(DocBody {
            body: Some(body_group),
            parts,
        })
    }

    /// Re-decode `document`, rebuild a multi-group [`elide::Report`]
    /// from the persisted body + parts, drive
    /// [`Orchestrator::anonymize_with`] with the reviewer overrides
    /// extracted from every group and the caller-filtered
    /// `policies`, and return the re-encoded redacted bytes.
    ///
    /// `policies` is the policy set already filtered by
    /// [`Policy::applies_when`] against the per-doc facts — the
    /// engine does not re-evaluate predicates. `spec` is the same
    /// [`AnalyzerParams`] that drove analyze (needed for the label
    /// catalog). `correlation_id` is server-minted, threaded into
    /// tracing spans on the redaction path.
    ///
    /// The body's modality (read from `body.body`'s [`EntityGroup`]
    /// variant) pins which typed handle the post-apply re-encode
    /// goes through — a container's body modality regardless of
    /// how many other modalities its parts ride on.
    ///
    /// [`Orchestrator::anonymize_with`]: elide::Orchestrator::anonymize_with
    /// [`Policy::applies_when`]: nvisy_core::policy::Policy::applies_when
    pub async fn apply_document(
        &self,
        document: RawDocument,
        spec: &AnalyzerParams,
        policies: &[Policy],
        body: &DocBody,
        correlation_id: Uuid,
    ) -> Result<ApplyOutcome> {
        let body_group = body.body.as_ref().ok_or_else(|| {
            Error::validation(
                "apply_document: body group is missing — analyze must run first",
                COMPONENT,
            )
        })?;
        let mut handle = self.decode(document).await?;
        let mut report = insert_body(elide::Report::new(), body_group);
        let mut overrides: Vec<(Uuid, RuleAction)> = Vec::new();
        collect_overrides_into(&mut overrides, body_group);
        for (id, group) in &body.parts {
            report = insert_part(report, id.as_str(), group);
            collect_overrides_into(&mut overrides, group);
        }

        let orchestrator = self.build_orchestrator(spec, policies, &overrides, correlation_id)?;
        orchestrator
            .anonymize_with(&mut handle, report)
            .await
            .map_err(|err| {
                Error::internal("orchestrator anonymize_with failed", COMPONENT).with_source(err)
            })?;

        encode_redacted(handle, body_group)
    }

    async fn decode(&self, document: RawDocument) -> Result<UntypedDocumentHandle> {
        let RawDocument {
            bytes, extension, ..
        } = document;
        self.formats
            .decode(bytes, extension.as_str())
            .await
            .map_err(|err| {
                Error::validation(
                    format!("codec decode failed for extension {extension:?}"),
                    COMPONENT,
                )
                .with_source(err)
            })
    }

    /// Build an [`Orchestrator`] with one pipeline per modality
    /// and a request-scoped [`Scope`].
    ///
    /// `policies` is the resolved policy set (empty during
    /// analyze). `overrides` are layered onto every modality's
    /// anonymizer ahead of the policy chain — entity ids are
    /// globally unique across body and parts, so an override
    /// matches in exactly one pipeline (the one whose recognized
    /// entities include that id) and is a no-op everywhere else.
    ///
    /// [`Scope`]: elide::recognition::Scope
    fn build_orchestrator(
        &self,
        spec: &AnalyzerParams,
        policies: &[Policy],
        overrides: &[(Uuid, RuleAction)],
        correlation_id: Uuid,
    ) -> Result<Orchestrator<'_>> {
        let catalog = spec.scope.label_catalog.compile();
        // Assemble the orchestrator's `Scope` from the three wire
        // knobs on `AnalyzerParams` + the caller-supplied
        // `correlation_id` + the resolved catalog. The catalog has
        // exactly one route (LabelCatalogParams); the correlation
        // id is server-minted (typically the run id) and never
        // appears on the wire shape.
        let scope = elide::recognition::Scope {
            languages: spec.scope.languages.clone(),
            countries: spec.scope.countries.clone(),
            labels: spec.scope.labels.clone(),
            catalog: catalog.clone(),
            correlation_id: Some(correlation_id),
        };

        let text_analyzer = spec.compile_text().map_err(compile_err)?;
        let tabular_analyzer = spec.compile_tabular().map_err(compile_err)?;
        let image_analyzer = spec.compile_image().map_err(compile_err)?;
        let audio_analyzer = spec.compile_audio().map_err(compile_err)?;

        // Build each modality's anonymizer fresh: start with the
        // catalog (so `with_tag` / `with_catalog_predicate` see
        // label tags), layer reviewer overrides, then attach the
        // policy chain so policy rules sit behind the overrides.
        let mut text_anonymizer = Anonymizer::<Text>::new().with_catalog(catalog.clone());
        for (id, action) in overrides {
            text_anonymizer =
                attach_override_text(text_anonymizer, *id, action).map_err(compile_err)?;
        }
        let text_anonymizer =
            attach_policies_text(text_anonymizer, policies.iter()).map_err(compile_err)?;

        let mut tabular_anonymizer = Anonymizer::<Tabular>::new().with_catalog(catalog.clone());
        for (id, action) in overrides {
            tabular_anonymizer =
                attach_override_tabular(tabular_anonymizer, *id, action).map_err(compile_err)?;
        }
        let tabular_anonymizer =
            attach_policies_tabular(tabular_anonymizer, policies.iter()).map_err(compile_err)?;

        let mut image_anonymizer = Anonymizer::<Image>::new().with_catalog(catalog.clone());
        for (id, action) in overrides {
            image_anonymizer = attach_override_image(image_anonymizer, *id, action);
        }
        let image_anonymizer = attach_policies_image(image_anonymizer, policies.iter());

        let mut audio_anonymizer = Anonymizer::<Audio>::new().with_catalog(catalog);
        for (id, action) in overrides {
            audio_anonymizer = attach_override_audio(audio_anonymizer, *id, action);
        }
        let audio_anonymizer = attach_policies_audio(audio_anonymizer, policies.iter());

        Ok(Orchestrator::new(&self.formats)
            .with_scope(scope)
            .with_modality::<Text>(text_analyzer, text_anonymizer)
            .with_modality::<Tabular>(tabular_analyzer, tabular_anonymizer)
            .with_modality::<Image>(image_analyzer, image_anonymizer)
            .with_modality::<Audio>(audio_analyzer, audio_anonymizer))
    }
}

/// Drain the body's entities from `report` into an
/// [`EntityGroup`] of the `Text` variant, or `None` if the body
/// is a different modality.
fn take_body_text(report: &mut elide::Report) -> Option<EntityGroup> {
    let entities = mem::take(report.entities::<Text>()?);
    Some(EntityGroup::Text {
        entities: entities.into_iter().map(EntityRecord::new).collect(),
    })
}

fn take_body_tabular(report: &mut elide::Report) -> Option<EntityGroup> {
    let entities = mem::take(report.entities::<Tabular>()?);
    Some(EntityGroup::Tabular {
        entities: entities.into_iter().map(EntityRecord::new).collect(),
    })
}

fn take_body_image(report: &mut elide::Report) -> Option<EntityGroup> {
    let entities = mem::take(report.entities::<Image>()?);
    Some(EntityGroup::Image {
        entities: entities.into_iter().map(EntityRecord::new).collect(),
    })
}

fn take_body_audio(report: &mut elide::Report) -> Option<EntityGroup> {
    let entities = mem::take(report.entities::<Audio>()?);
    Some(EntityGroup::Audio {
        entities: entities.into_iter().map(EntityRecord::new).collect(),
    })
}

fn take_part_text(report: &mut elide::Report, id: &PartId) -> Option<EntityGroup> {
    let entities = mem::take(report.part_entities::<Text>(id)?);
    Some(EntityGroup::Text {
        entities: entities.into_iter().map(EntityRecord::new).collect(),
    })
}

fn take_part_tabular(report: &mut elide::Report, id: &PartId) -> Option<EntityGroup> {
    let entities = mem::take(report.part_entities::<Tabular>(id)?);
    Some(EntityGroup::Tabular {
        entities: entities.into_iter().map(EntityRecord::new).collect(),
    })
}

fn take_part_image(report: &mut elide::Report, id: &PartId) -> Option<EntityGroup> {
    let entities = mem::take(report.part_entities::<Image>(id)?);
    Some(EntityGroup::Image {
        entities: entities.into_iter().map(EntityRecord::new).collect(),
    })
}

fn take_part_audio(report: &mut elide::Report, id: &PartId) -> Option<EntityGroup> {
    let entities = mem::take(report.part_entities::<Audio>(id)?);
    Some(EntityGroup::Audio {
        entities: entities.into_iter().map(EntityRecord::new).collect(),
    })
}

/// Insert the body group into `report` under its modality.
fn insert_body(report: elide::Report, group: &EntityGroup) -> elide::Report {
    match group {
        EntityGroup::Text { entities } => report.insert_body::<Text>(clone_entities(entities)),
        EntityGroup::Tabular { entities } => {
            report.insert_body::<Tabular>(clone_entities(entities))
        }
        EntityGroup::Image { entities } => report.insert_body::<Image>(clone_entities(entities)),
        EntityGroup::Audio { entities } => report.insert_body::<Audio>(clone_entities(entities)),
    }
}

/// Insert one part group into `report` under its modality.
fn insert_part(report: elide::Report, id: &str, group: &EntityGroup) -> elide::Report {
    let part_id = PartId::from(id.to_owned());
    match group {
        EntityGroup::Text { entities } => {
            report.insert_part::<Text>(part_id, clone_entities(entities))
        }
        EntityGroup::Tabular { entities } => {
            report.insert_part::<Tabular>(part_id, clone_entities(entities))
        }
        EntityGroup::Image { entities } => {
            report.insert_part::<Image>(part_id, clone_entities(entities))
        }
        EntityGroup::Audio { entities } => {
            report.insert_part::<Audio>(part_id, clone_entities(entities))
        }
    }
}

fn clone_entities<M: Modality>(records: &[EntityRecord<M>]) -> Vec<Entity<M>>
where
    Entity<M>: Clone,
{
    records.iter().map(|r| r.entity.clone()).collect()
}

/// Append every reviewer override on `group` to `out`. Iterates
/// the variant-appropriate `Vec<EntityRecord<M>>` and keeps only
/// records whose `override` field is set.
fn collect_overrides_into(out: &mut Vec<(Uuid, RuleAction)>, group: &EntityGroup) {
    match group {
        EntityGroup::Text { entities } => extend_overrides(out, entities),
        EntityGroup::Tabular { entities } => extend_overrides(out, entities),
        EntityGroup::Image { entities } => extend_overrides(out, entities),
        EntityGroup::Audio { entities } => extend_overrides(out, entities),
    }
}

fn extend_overrides<M: Modality>(
    out: &mut Vec<(Uuid, RuleAction)>,
    records: &[EntityRecord<M>],
) {
    out.extend(
        records
            .iter()
            .filter_map(|r| r.r#override.as_ref().map(|a| (r.entity.id, a.clone()))),
    );
}

/// After `anonymize_with` mutated `handle` in place, recover the
/// typed handle for the doc's body modality and re-encode it.
/// `handle` was a typed `DocumentHandle<M>` before being erased;
/// the apply-time re-encode needs the typed form because
/// [`elide::codec::DocumentHandle::encode`] is per-modality.
fn encode_redacted(handle: UntypedDocumentHandle, body: &EntityGroup) -> Result<ApplyOutcome> {
    match body {
        EntityGroup::Text { .. } => encode_typed::<Text>(handle, "Text"),
        EntityGroup::Tabular { .. } => encode_typed::<Tabular>(handle, "Tabular"),
        EntityGroup::Image { .. } => encode_typed::<Image>(handle, "Image"),
        EntityGroup::Audio { .. } => encode_typed::<Audio>(handle, "Audio"),
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

/// Translate an `elide::Error` from a `compile_*` call into the
/// runtime's error type. Compile failures are caller-driven (e.g.
/// unsupported recognizer for the modality), so they map to
/// [`Validation`].
///
/// [`Validation`]: nvisy_core::ErrorKind::Validation
fn compile_err(err: elide::Error) -> Error {
    Error::validation(format!("orchestrator compile failed: {err}"), COMPONENT).with_source(err)
}
