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

use std::mem;
use std::path::Path;
use std::sync::Arc;

use bytes::Bytes;
use elide::Orchestrator;
use elide::codec::{FormatRegistry, UntypedDocumentHandle};
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
use crate::runs::{DocBody, EntityRecord, ModalityKind};

const COMPONENT: &str = "engine";

/// Cheaply-cloneable runtime adapter: persistence + codecs + the
/// per-request orchestrator constructor.
#[derive(Clone)]
pub struct Engine {
    registry: RegistryHandle,
    formats: Arc<FormatRegistry>,
}

/// Outcome of analyzing one document end-to-end.
pub struct AnalyzeOutcome {
    /// Modality elide's codec resolved the bytes to.
    pub modality: ModalityKind,
    /// Recognized body entities, wrapped in [`EntityRecord`] for
    /// persistence (no overrides set yet — those flow through the
    /// reviewer surface).
    pub body: DocBody,
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
    /// the body entities of the returned [`elide::Report`] onto
    /// the persistence-shaped [`AnalyzeOutcome`].
    ///
    /// Container-part entities (PDF embedded images, archive
    /// members, …) are detected by the orchestrator but discarded
    /// at the persistence boundary today; following slices will
    /// evolve [`DocBody`] to retain them.
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
    ) -> Result<AnalyzeOutcome> {
        let extension = document.extension.clone();
        let mut handle = self.decode(document).await?;
        let orchestrator = self.build_orchestrator(spec, &[], None, &[], correlation_id)?;
        let mut report = orchestrator.analyze(&mut handle).await.map_err(|err| {
            Error::internal("orchestrator analyze failed", COMPONENT).with_source(err)
        })?;

        // The orchestrator decided which pipeline matched the body
        // (one of Text / Tabular / Image / Audio) — recover that by
        // peeking at the report's body entity slot for each
        // modality.
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

    /// Re-decode `document`, rebuild a body-only [`elide::Report`]
    /// from the persisted `body` entities, drive
    /// [`Orchestrator::anonymize_with`] with the reviewer overrides
    /// extracted from those entities and the caller-filtered
    /// `policies`, and return the re-encoded redacted bytes.
    ///
    /// `policies` is the policy set already filtered by
    /// [`Policy::applies_when`] against the per-doc facts — the
    /// engine does not re-evaluate predicates. `spec` is the same
    /// [`AnalyzerParams`] that drove analyze (needed for the label
    /// catalog). `correlation_id` is server-minted, threaded into
    /// tracing spans on the redaction path.
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
        let mut handle = self.decode(document).await?;
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

        let orchestrator =
            self.build_orchestrator(spec, policies, Some(modality), &overrides, correlation_id)?;
        orchestrator
            .anonymize_with(&mut handle, report)
            .await
            .map_err(|err| {
                Error::internal("orchestrator anonymize_with failed", COMPONENT).with_source(err)
            })?;

        encode_redacted(handle, modality)
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
    /// analyze). When `body_modality` is `Some`, `overrides` are
    /// layered onto that modality's anonymizer ahead of the
    /// policy chain; on the other three modalities the overrides
    /// have no effect.
    ///
    /// [`Scope`]: elide::recognition::Scope
    fn build_orchestrator(
        &self,
        spec: &AnalyzerParams,
        policies: &[Policy],
        body_modality: Option<ModalityKind>,
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
        // label tags), layer reviewer overrides for the body
        // modality only, then attach the policy chain so policy
        // rules sit behind the overrides.
        let body_overrides = |kind| {
            body_modality
                .filter(|m| *m == kind)
                .map(|_| overrides)
                .unwrap_or(&[][..])
        };

        let mut text_anonymizer = Anonymizer::<Text>::new().with_catalog(catalog.clone());
        for (id, action) in body_overrides(ModalityKind::Text) {
            text_anonymizer =
                attach_override_text(text_anonymizer, *id, action).map_err(compile_err)?;
        }
        let text_anonymizer =
            attach_policies_text(text_anonymizer, policies.iter()).map_err(compile_err)?;

        let mut tabular_anonymizer = Anonymizer::<Tabular>::new().with_catalog(catalog.clone());
        for (id, action) in body_overrides(ModalityKind::Tabular) {
            tabular_anonymizer =
                attach_override_tabular(tabular_anonymizer, *id, action).map_err(compile_err)?;
        }
        let tabular_anonymizer =
            attach_policies_tabular(tabular_anonymizer, policies.iter()).map_err(compile_err)?;

        let mut image_anonymizer = Anonymizer::<Image>::new().with_catalog(catalog.clone());
        for (id, action) in body_overrides(ModalityKind::Image) {
            image_anonymizer = attach_override_image(image_anonymizer, *id, action);
        }
        let image_anonymizer = attach_policies_image(image_anonymizer, policies.iter());

        let mut audio_anonymizer = Anonymizer::<Audio>::new().with_catalog(catalog);
        for (id, action) in body_overrides(ModalityKind::Audio) {
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

/// Translate an `elide::Error` from a `compile_*` call into the
/// runtime's error type. Compile failures are caller-driven (e.g.
/// unsupported recognizer for the modality), so they map to
/// [`Validation`].
///
/// [`Validation`]: nvisy_core::ErrorKind::Validation
fn compile_err(err: elide::Error) -> Error {
    Error::validation(format!("orchestrator compile failed: {err}"), COMPONENT).with_source(err)
}
