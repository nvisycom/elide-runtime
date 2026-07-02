//! [`Engine`]: the stateless pipeline over [`elide`].
//!
//! Long-lived state (only two things):
//!
//! - The [`FormatRegistry`] over elide's codec set. Decodes raw
//!   bytes into a modality-typed [`DocumentHandle`] at analyze +
//!   apply time.
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
//!   [`Orchestrator`] with one pipeline per modality + the request
//!   scope, runs detection, and projects the report (body + every
//!   container part) onto the caller-facing [`DocBody`].
//! - [`Engine::apply_document`] decodes raw bytes again, rebuilds
//!   a multi-group [`Report`] from the returned body + parts,
//!   layers the reviewer overrides + filtered policy set onto
//!   each modality's anonymizer, drives redaction, and returns
//!   the re-encoded bytes via [`ApplyOutcome`].
//!
//! Both methods build a fresh [`Orchestrator`] per call: it is a
//! small map of trait objects keyed by modality `TypeId`, cheap
//! to construct. The per-call shape lets us re-resolve policies +
//! scope per document at apply time without mutating a shared
//! anonymizer.
//!
//! Hosts hold the returned [`DocBody`] between analyze and apply
//! however they see fit — in memory, in a run store, in a
//! reviewer UI's state — and hand it back to `apply_document`
//! with any per-entity reviewer overrides folded in.
//!
//! ## Internal layout
//!
//! - [`analyzer`] / [`anonymizer`] compile per-modality `spec`
//!   and `policies` into per-modality elide types.
//! - [`orchestrator`] wires those into an [`Orchestrator`] for a
//!   single request.
//! - [`report`] translates between elide's runtime [`Report`] and
//!   the caller-facing [`DocBody`] / [`RecognizedGroup`].
//! - [`document`] defines [`DocBody`] and [`RecognizedGroup`],
//!   the analyze → apply bridge.
//!
//! [`DocBody`]: document::DocBody
//! [`RecognizedGroup`]: document::RecognizedGroup
//! [`FormatRegistry`]: elide::codec::FormatRegistry
//! [`DocumentHandle`]: elide::codec::DocumentHandle
//! [`Orchestrator`]: elide::Orchestrator
//! [`Report`]: elide::Report

pub(crate) mod analyzer;
pub(crate) mod anonymizer;
pub mod document;
mod orchestrator;
mod report;

use std::any::TypeId;
use std::collections::HashMap;
use std::sync::Arc;

use bytes::Bytes;
use elide::codec::{FormatRegistry, PartId, UntypedDocumentHandle};
use elide_core::modality::audio::Audio;
use elide_core::modality::image::Image;
use elide_core::modality::tabular::Tabular;
use elide_core::modality::text::Text;
use nvisy_core::{Error, Result};
use nvisy_schema::file::RawDocument;
use nvisy_schema::plan::AnalyzerParams;
use nvisy_schema::policy::{Policy, RuleAction};
use uuid::Uuid;

use self::document::DocBody;
use self::report::{
    collect_overrides_into, encode_redacted, insert_body, insert_part, take_body, take_part,
};

const COMPONENT: &str = "engine";

/// Cheaply-cloneable pipeline adapter: codec registry + the
/// deployment's NER / LLM lineups + the per-request orchestrator
/// constructor.
#[derive(Clone, Default)]
pub struct Engine {
    formats: Arc<FormatRegistry>,
    ner: Arc<nvisy_core::ner::NerConfig>,
    llm: Arc<nvisy_core::llm::LlmConfig>,
}

/// Outcome of applying redactions to one document.
#[derive(Debug, Clone)]
pub struct ApplyOutcome {
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
    /// the report onto the caller-facing [`DocBody`].
    ///
    /// Captures the body group *and* every container part group
    /// (DOCX embedded images, archive members, ...) the
    /// orchestrator returned; each returned group carries its
    /// own modality tag via its [`RecognizedGroup`] variant.
    ///
    /// `correlation_id` is caller-minted (typically a per-run or
    /// per-request id); the orchestrator threads it into tracing
    /// spans on the detection path.
    ///
    /// [`Orchestrator::analyze`]: elide::Orchestrator::analyze
    /// [`RecognizedGroup`]: document::RecognizedGroup
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
        let body_group = take_body::<Text>(&mut report)
            .or_else(|| take_body::<Tabular>(&mut report))
            .or_else(|| take_body::<Image>(&mut report))
            .or_else(|| take_body::<Audio>(&mut report))
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
                take_part::<Text>(&mut report, &id)
            } else if type_id == TypeId::of::<Tabular>() {
                take_part::<Tabular>(&mut report, &id)
            } else if type_id == TypeId::of::<Image>() {
                take_part::<Image>(&mut report, &id)
            } else if type_id == TypeId::of::<Audio>() {
                take_part::<Audio>(&mut report, &id)
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

    /// Re-decode `document`, rebuild a multi-group
    /// [`elide::Report`] from the analyze-returned body + parts,
    /// drive [`Orchestrator::anonymize_with`] with the reviewer
    /// overrides extracted from every group and the caller-filtered
    /// `policies`, and return the re-encoded redacted bytes.
    ///
    /// `policies` is the policy set already filtered by
    /// [`Policy::applies_when`] against the per-doc facts; the
    /// engine does not re-evaluate predicates. `spec` is the same
    /// [`AnalyzerParams`] that drove analyze (needed for the label
    /// catalog). `correlation_id` is caller-minted, threaded into
    /// tracing spans on the redaction path.
    ///
    /// The body's modality (read from `body.body`'s
    /// [`RecognizedGroup`] variant) pins which typed handle the
    /// post-apply re-encode goes through: a container's body
    /// modality regardless of how many other modalities its parts
    /// ride on.
    ///
    /// [`Orchestrator::anonymize_with`]: elide::Orchestrator::anonymize_with
    /// [`Policy::applies_when`]: nvisy_schema::policy::Policy::applies_when
    /// [`RecognizedGroup`]: document::RecognizedGroup
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
}
