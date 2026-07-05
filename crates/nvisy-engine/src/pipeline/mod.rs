//! [`Engine`]: the stateless pipeline over [`elide`].
//!
//! Long-lived state (only two things):
//!
//! - The [`FormatRegistry`] over elide's codec set. Decodes raw
//!   bytes into a modality-typed [`DocumentHandle`] at analyze +
//!   anonymize time.
//! - The deployment's NER + LLM lineups (see [`nvisy_core::ner`]
//!   and [`nvisy_core::llm`]). Consulted by the analyzer compile
//!   whenever the request's `AnalyzerParams.recognizers.ner` or
//!   `.llm` toggle is on.
//!
//! [`Engine`] clones cheaply (`Arc` under the hood). Callers pass
//! a clone into every request-scoped code path they run.
//!
//! ## Per-document verbs
//!
//! - [`Engine::analyze_document`] decodes raw bytes, builds an
//!   [`Orchestrator`] with one pipeline per modality + the
//!   request scope, runs detection, and projects the report
//!   (body + every container part) onto the caller-facing
//!   [`AnalyzedDocument`].
//! - [`Engine::anonymize_document`] decodes raw bytes again,
//!   rebuilds a multi-group [`Report`] from the returned body +
//!   parts, layers the reviewer overrides + filtered policy set
//!   onto each modality's anonymizer, drives redaction, and
//!   returns the re-encoded bytes via [`AnonymizedDocument`].
//!
//! Both methods build a fresh [`Orchestrator`] per call: it is a
//! small map of trait objects keyed by modality `TypeId`, cheap
//! to construct. The per-call shape lets us re-resolve policies
//! per document at anonymize time without mutating a shared
//! anonymizer.
//!
//! The recognition [`Scope`] (label catalog + asserted
//! languages, countries, and document labels) is compiled once
//! by analyze and persisted onto [`AnalyzedDocument::scope`];
//! anonymize reads it from there. Callers do not re-pass an
//! [`AnalyzerParams`] to anonymize — analyze's vocabulary and
//! anonymize's vocabulary are the same by construction.
//!
//! Hosts hold the returned [`AnalyzedDocument`] between analyze
//! and anonymize however they see fit — in memory, in a run
//! store, in a reviewer UI's state — and hand it back to
//! `anonymize_document` with any per-entity reviewer overrides
//! folded in.
//!
//! [`Scope`]: elide::recognition::Scope
//!
//! ## Internal layout
//!
//! Sibling crate-level modules provide the modality-shaped
//! plumbing:
//!
//! - `crate::analyzer` and `crate::anonymizer` compile
//!   per-modality `spec` and `policies` into per-modality elide
//!   types.
//!
//! Inside this module:
//!
//! - `orchestrator` wires those into an [`Orchestrator`] for a
//!   single request.
//! - `report` translates between elide's runtime [`Report`] and
//!   the caller-facing [`AnalyzedDocument`] / [`RecognizedGroup`].
//! - `analyzed` defines [`AnalyzedDocument`] and
//!   [`RecognizedGroup`], the analyze → anonymize bridge; both
//!   types re-exported at the crate root.
//!
//! [`AnalyzedDocument`]: crate::AnalyzedDocument
//! [`RecognizedGroup`]: crate::RecognizedGroup
//! [`FormatRegistry`]: elide::codec::FormatRegistry
//! [`DocumentHandle`]: elide::codec::DocumentHandle
//! [`Orchestrator`]: elide::Orchestrator
//! [`Report`]: elide::Report

mod analyzed;
mod orchestrator;
mod report;

use std::any::TypeId;
use std::collections::HashMap;
use std::sync::Arc;

use bytes::Bytes;
use elide::Report;
use elide::codec::{FormatRegistry, PartId, UntypedDocumentHandle};
#[cfg(feature = "internal_audio")]
use elide_core::modality::audio::Audio;
#[cfg(feature = "internal_image")]
use elide_core::modality::image::Image;
#[cfg(feature = "internal_tabular")]
use elide_core::modality::tabular::Tabular;
use elide_core::modality::text::Text;
use nvisy_core::{Error, Result};
use nvisy_schema::context::Context;
use nvisy_schema::file::Document;
use nvisy_schema::plan::AnalyzerParams;
use nvisy_schema::policy::{Policy, PolicyAction};
use uuid::Uuid;

pub use self::analyzed::{AnalyzedDocument, EntityRecord, RecognizedGroup};
use self::report::{
    collect_overrides_into, encode_redacted, insert_body, insert_part, take_body, take_part,
};

const COMPONENT: &str = "pipeline";

/// Cheaply-cloneable pipeline adapter: codec registry + the
/// deployment's NER / LLM lineups + the per-request orchestrator
/// constructor.
#[derive(Clone, Default)]
pub struct Engine {
    formats: Arc<FormatRegistry>,
    ner: Arc<nvisy_core::ner::NerConfig>,
    llm: Arc<nvisy_core::llm::LlmConfig>,
}

/// The redacted output of [`Engine::anonymize_document`]: the
/// re-encoded document bytes after every applicable redaction
/// operator ran.
#[derive(Debug, Clone)]
pub struct AnonymizedDocument {
    /// Encoded bytes of the redacted document.
    pub bytes: Bytes,
}

impl Engine {
    /// New engine paired with elide's built-in codec set
    /// ([`FormatRegistry::with_builtin`]) plus empty NER and LLM
    /// lineups. Callers that want NER or LLM recognition must
    /// chain [`with_ner`](Self::with_ner) or
    /// [`with_llm`](Self::with_llm).
    pub fn new() -> Self {
        Self {
            formats: Arc::new(FormatRegistry::with_builtin()),
            ner: Arc::new(nvisy_core::ner::NerConfig::default()),
            llm: Arc::new(nvisy_core::llm::LlmConfig::default()),
        }
    }

    /// Set the deployment's NER configuration. Consumed once at
    /// setup; the analyzer compile reads it every time a request
    /// submits `AnalyzerParams.recognizers.ner = true`.
    #[must_use]
    pub fn with_ner(mut self, ner: nvisy_core::ner::NerConfig) -> Self {
        self.ner = Arc::new(ner);
        self
    }

    /// Set the deployment's LLM configuration. Consumed once at
    /// setup; the analyzer compile reads it every time a request
    /// submits `AnalyzerParams.recognizers.llm = true`.
    #[must_use]
    pub fn with_llm(mut self, llm: nvisy_core::llm::LlmConfig) -> Self {
        self.llm = Arc::new(llm);
        self
    }

    /// The codec registry. Pipeline calls reach for it to decode
    /// raw bytes into an [`UntypedDocumentHandle`].
    pub fn formats(&self) -> &FormatRegistry {
        &self.formats
    }

    /// Decode `document`, drive [`Orchestrator::analyze`], project
    /// the report onto the caller-facing [`AnalyzedDocument`].
    ///
    /// Captures the body group *and* every container part group
    /// (DOCX embedded images, archive members, ...) the
    /// orchestrator returned; each returned group carries its
    /// own modality tag via its [`RecognizedGroup`] variant.
    ///
    /// `contexts` is a placeholder for the deployment's
    /// reference-data collections. Reserved on the API: no
    /// built-in recognizer in this workspace consumes contexts
    /// yet — see
    /// <https://github.com/nvisycom/runtime/issues/314> for the
    /// wiring plan.
    ///
    /// [`Orchestrator::analyze`]: elide::Orchestrator::analyze
    /// [`RecognizedGroup`]: crate::RecognizedGroup
    pub async fn analyze_document(
        &self,
        document: Document,
        spec: &AnalyzerParams,
        contexts: &[Context],
    ) -> Result<AnalyzedDocument> {
        let _ = contexts;
        let correlation_id = document.correlation_id;
        let extension = document.extension.clone();
        let mut handle = self.decode(document).await?;
        let (orchestrator, scope) = self.build_analyze_orchestrator(spec, correlation_id)?;
        let mut report = orchestrator.analyze(&mut handle).await.map_err(|err| {
            Error::internal("orchestrator analyze failed", COMPONENT).with_source(err)
        })?;

        // Walk the body modality slots in order; the first that
        // returns Some is the body modality the orchestrator's
        // codec resolved. Only the modalities compiled in are
        // considered; a document decoded to a disabled modality
        // ends up in the `None` arm and surfaces as Validation.
        let body_group = take_body::<Text>(&mut report);
        #[cfg(feature = "internal_tabular")]
        let body_group = body_group.or_else(|| take_body::<Tabular>(&mut report));
        #[cfg(feature = "internal_image")]
        let body_group = body_group.or_else(|| take_body::<Image>(&mut report));
        #[cfg(feature = "internal_audio")]
        let body_group = body_group.or_else(|| take_body::<Audio>(&mut report));

        let body_group = body_group.ok_or_else(|| {
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
            let group = take_part_dispatch(&mut report, &id, type_id);
            if let Some(group) = group {
                parts.insert(id.as_str().to_owned(), group);
            }
        }

        Ok(AnalyzedDocument {
            body: Some(body_group),
            parts,
            scope,
        })
    }

    /// Re-decode `document`, rebuild a multi-group [`Report`]
    /// from the analyze-returned body + parts, drive
    /// [`Orchestrator::anonymize_with`] with the reviewer
    /// overrides extracted from every group and the
    /// caller-filtered `policies`, and return the re-encoded
    /// redacted bytes.
    ///
    /// `policies` is the policy set already filtered by
    /// [`Policy::applies_when`] against the per-doc facts; the
    /// engine does not re-evaluate predicates. The vocabulary
    /// the anonymizer compiles against (label catalog, asserted
    /// languages / jurisdictions / labels) travels on
    /// [`analyzed.scope`], persisted by analyze — the caller
    /// does not re-pass an [`AnalyzerParams`]. The document's
    /// `correlation_id` is threaded into tracing spans on the
    /// redaction path.
    ///
    /// The body's modality (read from `analyzed.body`'s
    /// [`RecognizedGroup`] variant) pins which typed handle the
    /// post-apply re-encode goes through: a container's body
    /// modality regardless of how many other modalities its parts
    /// ride on.
    ///
    /// [`Orchestrator::anonymize_with`]: elide::Orchestrator::anonymize_with
    /// [`Policy::applies_when`]: nvisy_schema::policy::Policy::applies_when
    /// [`RecognizedGroup`]: crate::RecognizedGroup
    /// [`analyzed.scope`]: AnalyzedDocument::scope
    pub async fn anonymize_document(
        &self,
        document: Document,
        policies: &[Policy],
        analyzed: &AnalyzedDocument,
    ) -> Result<AnonymizedDocument> {
        let body_group = analyzed.body.as_ref().ok_or_else(|| {
            Error::validation(
                "anonymize_document: body group is missing — analyze must run first",
                COMPONENT,
            )
        })?;
        let correlation_id = document.correlation_id;
        let mut handle = self.decode(document).await?;
        let mut report = insert_body(Report::new(), body_group);
        let mut overrides: Vec<(Uuid, PolicyAction)> = Vec::new();
        collect_overrides_into(&mut overrides, body_group);
        for (id, group) in &analyzed.parts {
            report = insert_part(report, id.as_str(), group);
            collect_overrides_into(&mut overrides, group);
        }

        let orchestrator = self.build_anonymize_orchestrator(
            &analyzed.scope,
            policies,
            &overrides,
            correlation_id,
        )?;
        orchestrator
            .anonymize_with(&mut handle, report)
            .await
            .map_err(|err| {
                Error::internal("orchestrator anonymize_with failed", COMPONENT).with_source(err)
            })?;

        encode_redacted(handle, body_group)
    }

    async fn decode(&self, document: Document) -> Result<UntypedDocumentHandle> {
        let Document {
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
}

/// Dispatch a part to the take-part helper matching its modality
/// `TypeId`. Feature-gated per modality; a part whose modality is
/// disabled in this build falls through to `None`, and the caller
/// treats it the same as a part the engine doesn't model.
fn take_part_dispatch(
    report: &mut Report,
    id: &PartId,
    type_id: TypeId,
) -> Option<RecognizedGroup> {
    if type_id == TypeId::of::<Text>() {
        return take_part::<Text>(report, id);
    }
    #[cfg(feature = "internal_tabular")]
    if type_id == TypeId::of::<Tabular>() {
        return take_part::<Tabular>(report, id);
    }
    #[cfg(feature = "internal_image")]
    if type_id == TypeId::of::<Image>() {
        return take_part::<Image>(report, id);
    }
    #[cfg(feature = "internal_audio")]
    if type_id == TypeId::of::<Audio>() {
        return take_part::<Audio>(report, id);
    }
    None
}
