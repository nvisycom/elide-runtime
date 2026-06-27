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
//! - [`Engine::analyze`] builds a [`Orchestrator`] with one
//!   pipeline per modality + the request scope, then runs its
//!   detection phase. Returns the editable [`Report`].
//! - [`Engine::anonymize_with`] builds the same orchestrator with
//!   `policies` layered onto each modality's anonymizer (plus
//!   reviewer overrides on the body modality), then runs the
//!   redaction phase against a (possibly edited) [`Report`].
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
pub(crate) mod scope;

use std::path::Path;
use std::sync::Arc;

use elide::Orchestrator;
use elide::codec::{FormatRegistry, UntypedDocumentHandle};
use elide::redaction::Anonymizer;
use elide_core::modality::audio::Audio;
use elide_core::modality::image::Image;
use elide_core::modality::tabular::Tabular;
use elide_core::modality::text::Text;
use nvisy_core::plan::AnalyzerParams;
use nvisy_core::policy::{Policy, RuleAction};
use nvisy_core::{Error, Result};
use uuid::Uuid;

use self::analyzer::{build_catalog, compile_audio, compile_image, compile_tabular, compile_text};
use self::anonymizer::{
    attach_override_audio, attach_override_image, attach_override_tabular, attach_override_text,
    attach_policies_audio, attach_policies_image, attach_policies_tabular, attach_policies_text,
};
use self::scope::compile_scope;
use crate::registry::RegistryHandle;
use crate::runs::ModalityKind;

const COMPONENT: &str = "engine";

/// Cheaply-cloneable runtime adapter: persistence + codecs + the
/// per-request orchestrator constructor.
#[derive(Clone)]
pub struct Engine {
    registry: RegistryHandle,
    formats: Arc<FormatRegistry>,
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

    /// Drive [`Orchestrator::analyze`] over one handle.
    ///
    /// Compiles the per-modality pipelines from `spec` (with
    /// empty anonymizers — apply isn't called), registers all
    /// four on a fresh [`Orchestrator`] alongside the request
    /// [`Scope`], then walks `handle`'s body and any container
    /// parts. Returns the editable [`elide::Report`].
    ///
    /// [`Orchestrator::analyze`]: elide::Orchestrator::analyze
    /// [`Scope`]: elide::recognition::Scope
    pub async fn analyze(
        &self,
        handle: &mut UntypedDocumentHandle,
        spec: &AnalyzerParams,
    ) -> Result<elide::Report> {
        let orchestrator = self.build_orchestrator(spec, &[], None, &[])?;
        orchestrator.analyze(handle).await.map_err(|err| {
            Error::internal("orchestrator analyze failed", COMPONENT).with_source(err)
        })
    }

    /// Drive [`Orchestrator::anonymize_with`] with a (possibly
    /// edited) [`elide::Report`].
    ///
    /// Compiles the per-modality pipelines from `spec` *and*
    /// `policies`, layering `overrides` onto the body modality
    /// (overrides on other modalities don't apply to this doc),
    /// registers all four on a fresh [`Orchestrator`] alongside
    /// the request [`Scope`], and re-drives `handle` against the
    /// report.
    ///
    /// `body_modality` pins which modality the overrides target —
    /// the document has exactly one body modality, and reviewer
    /// overrides are by definition on that body's entities.
    ///
    /// [`Orchestrator::anonymize_with`]: elide::Orchestrator::anonymize_with
    /// [`Scope`]: elide::recognition::Scope
    pub async fn anonymize_with(
        &self,
        handle: &mut UntypedDocumentHandle,
        spec: &AnalyzerParams,
        policies: &[Policy],
        body_modality: ModalityKind,
        overrides: &[(Uuid, RuleAction)],
        report: elide::Report,
    ) -> Result<()> {
        let orchestrator =
            self.build_orchestrator(spec, policies, Some(body_modality), overrides)?;
        orchestrator
            .anonymize_with(handle, report)
            .await
            .map_err(|err| {
                Error::internal("orchestrator anonymize_with failed", COMPONENT).with_source(err)
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
    fn build_orchestrator(
        &self,
        spec: &AnalyzerParams,
        policies: &[Policy],
        body_modality: Option<ModalityKind>,
        overrides: &[(Uuid, RuleAction)],
    ) -> Result<Orchestrator<'_>> {
        let catalog = build_catalog(spec);
        let scope = compile_scope(&spec.scope, catalog.clone()).map_err(compile_err)?;

        let text_analyzer = compile_text(spec).map_err(compile_err)?;
        let tabular_analyzer = compile_tabular(spec).map_err(compile_err)?;
        let image_analyzer = compile_image(spec).map_err(compile_err)?;
        let audio_analyzer = compile_audio(spec).map_err(compile_err)?;

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

        let mut tabular_anonymizer =
            Anonymizer::<Tabular>::new().with_catalog(catalog.clone());
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

/// Translate an `elide::Error` from a `compile_*` call into the
/// runtime's error type. Compile failures are caller-driven (e.g.
/// unsupported recognizer for the modality), so they map to
/// [`Validation`].
///
/// [`Validation`]: nvisy_core::ErrorKind::Validation
fn compile_err(err: elide::Error) -> Error {
    Error::validation(format!("orchestrator compile failed: {err}"), COMPONENT).with_source(err)
}
