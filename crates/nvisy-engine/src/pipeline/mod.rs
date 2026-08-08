//! [`Engine`]: the stateless pipeline over [`elide`].
//!
//! Long-lived state (only two things):
//!
//! - The [`FormatRegistry`] over elide's codec set. Decodes raw
//!   bytes into a modality-typed [`DocumentHandle`] at analyze +
//!   anonymize time.
//! - The deployment's NER + LLM lineups (see [`crate::provider::ner`]
//!   and [`crate::provider::llm`]). Consulted by the analyzer
//!   compile whenever the request's
//!   `AnalyzerParams.recognizers.{ner,llm}` selects any recognizer.
//!
//! [`Engine`] clones cheaply (`Arc` under the hood). Callers pass
//! a clone into every request-scoped code path they run.
//!
//! ## Per-document verbs
//!
//! - [`Engine::analyze`] decodes raw bytes, builds an
//!   [`Orchestrator`] with one pipeline per modality + the
//!   request scope, runs detection, and projects the report
//!   (body + every container part) onto the caller-facing
//!   [`Audit`].
//! - [`Engine::anonymize`] decodes raw bytes again, rebuilds a
//!   multi-group [`Report`] from the returned body + parts,
//!   layers the reviewer overrides + filtered policy set onto
//!   each modality's anonymizer, drives redaction, and returns
//!   the re-encoded [`Document`].
//!
//! Both methods build a fresh [`Orchestrator`] per call: it is a
//! small map of trait objects keyed by modality `TypeId`, cheap
//! to construct. The per-call shape lets us re-resolve policies
//! per document at anonymize time without mutating a shared
//! anonymizer.
//!
//! The recognition scope split has two owners: caller-asserted
//! facts (languages, jurisdictions, tags) travel between calls on
//! [`Audit::context`]; the label catalog is derived from
//! `policies` afresh on every call so policies stay the single
//! source of truth for label vocabulary. Callers do not re-pass
//! an [`AnalyzerParams`] to anonymize.
//!
//! Hosts hold the returned [`Audit`] between analyze and
//! anonymize however they see fit — in memory, in a run store, in
//! a reviewer UI's state — and hand it back to
//! [`Engine::anonymize`] with any per-entity reviewer overrides
//! folded in.
//!
//! ## Internal layout
//!
//! Sibling crate-level modules provide the modality-shaped
//! plumbing:
//!
//! - `crate::analyzer` and `crate::anonymizer` compile
//!   per-modality `spec` and `policies` into per-modality elide
//!   types.
//! - `crate::entity` owns [`EntityGroup`] (the modality-tagged
//!   entity carrier) and its report bridging helpers.
//!
//! Inside this module:
//!
//! - `orchestrator` wires those into an [`Orchestrator`] for a
//!   single request.
//! - `audit` defines [`Audit`], [`AuditContext`], the analyze →
//!   anonymize bridge; all re-exported at the crate root.
//!
//! [`Audit`]: crate::Audit
//! [`AuditContext`]: crate::AuditContext
//! [`EntityGroup`]: crate::entity::EntityGroup
//! [`FormatRegistry`]: elide::codec::FormatRegistry
//! [`DocumentHandle`]: elide::codec::DocumentHandle
//! [`Orchestrator`]: elide::Orchestrator
//! [`Report`]: elide::Report

mod audit;
#[cfg(feature = "audit-csv")]
mod audit_csv;
mod orchestrator;
mod registered;

use std::any::TypeId;
use std::collections::HashMap;
use std::sync::Arc;

use elide::codec::{FormatRegistry, PartId, UntypedDocumentHandle};
use elide::redaction::operators::KeyProvider;
use elide::{Directives, Report};
#[cfg(feature = "internal_audio")]
use elide_core::modality::audio::Audio;
#[cfg(feature = "internal_image")]
use elide_core::modality::image::Image;
#[cfg(feature = "internal_tabular")]
use elide_core::modality::tabular::Tabular;
use elide_core::modality::text::Text;
use elide_core::{Error, ErrorKind, Result};
use nvisy_schema::file::Document;
use nvisy_schema::plan::AnalyzerParams;
use nvisy_schema::policy::PolicyDefinition;
use nvisy_schema::policy::redaction::ModalityRedactions;
use uuid::Uuid;

pub use self::audit::{Audit, AuditContext};
pub use self::registered::RegisteredRecognizer;
use crate::PatternGuardrails;
use crate::entity::{EntityGroup, take_body, take_part};
use crate::provider::llm::LlmConfig;
use crate::provider::ner::NerConfig;

/// Cheaply-cloneable pipeline adapter over [`elide`].
///
/// Bundles the codec registry, the deployment's NER / LLM
/// lineups, the pattern-recognizer guardrails, the shared
/// [`KeyProvider`] (for `HmacHash` and `Encrypt`), and the
/// per-request orchestrator constructor.
///
/// [`KeyProvider`]: elide::redaction::operators::KeyProvider
#[derive(Clone, Default)]
pub struct Engine {
    formats: Arc<FormatRegistry>,
    ner: Arc<NerConfig>,
    llm: Arc<LlmConfig>,
    pattern_guardrails: PatternGuardrails,
    pub(super) key_provider: Option<Arc<dyn KeyProvider>>,
}

impl Engine {
    /// New engine paired with elide's built-in codec set.
    ///
    /// Uses [`FormatRegistry::with_builtin`] plus empty NER and
    /// LLM lineups. Callers that want NER or LLM recognition
    /// must chain [`with_ner`] or [`with_llm`]. Callers whose
    /// policies use `HmacHash` or `Encrypt` must wire a key
    /// provider via [`with_key_provider`].
    ///
    /// [`with_ner`]: Self::with_ner
    /// [`with_llm`]: Self::with_llm
    /// [`with_key_provider`]: Self::with_key_provider
    pub fn new() -> Self {
        Self {
            formats: Arc::new(FormatRegistry::with_builtin()),
            ner: Arc::new(NerConfig::default()),
            llm: Arc::new(LlmConfig::default()),
            pattern_guardrails: PatternGuardrails::default(),
            key_provider: None,
        }
    }

    /// Set the deployment's NER configuration.
    ///
    /// Consumed once at setup; the analyzer compile reads it on
    /// every request whose `AnalyzerParams.recognizers.ner`
    /// selects any of the configured recognizers.
    #[must_use]
    pub fn with_ner(mut self, ner: NerConfig) -> Self {
        self.ner = Arc::new(ner);
        self
    }

    /// Set the deployment's LLM configuration.
    ///
    /// Consumed once at setup; the analyzer compile reads it on
    /// every request whose `AnalyzerParams.recognizers.llm`
    /// selects any of the configured recognizers.
    #[must_use]
    pub fn with_llm(mut self, llm: LlmConfig) -> Self {
        self.llm = Arc::new(llm);
        self
    }

    /// Set the shared cryptographic [`KeyProvider`] the
    /// `HmacHash` and `Encrypt` operators resolve their keys
    /// through.
    ///
    /// One provider backs both operators; per-label keys are the
    /// provider's own responsibility. A policy that names either
    /// operator without a provider wired errors at request
    /// compile time — the audit trail names the operator so a
    /// misconfiguration surfaces at load, not silently.
    ///
    /// [`KeyProvider`]: elide::redaction::operators::KeyProvider
    #[must_use]
    pub fn with_key_provider(mut self, provider: Arc<dyn KeyProvider>) -> Self {
        self.key_provider = Some(provider);
        self
    }

    /// Set the pattern-recognizer guardrails.
    ///
    /// Bounds the ReDoS attack surface and automaton compile
    /// cost when callers inline custom regex rules and
    /// dictionaries on
    /// [`PatternRecognizerParams`]. `max_regex_source_len` is
    /// clamped to the wire-layer ceiling on construction; every
    /// other knob applies as-is at analyzer-compile time.
    ///
    /// [`PatternRecognizerParams`]: nvisy_schema::plan::PatternRecognizerParams
    #[must_use]
    pub fn with_pattern_guardrails(mut self, guardrails: PatternGuardrails) -> Self {
        self.pattern_guardrails = guardrails.clamped();
        self
    }

    /// The codec registry.
    ///
    /// Pipeline calls reach for it to decode raw bytes into an
    /// [`UntypedDocumentHandle`].
    pub fn formats(&self) -> &FormatRegistry {
        &self.formats
    }

    /// Every NER recognizer this engine has registered, in
    /// configuration order.
    ///
    /// Feeds a "list recognizers" endpoint. Each entry carries
    /// name, optional description, and provider slug; connection
    /// details and (future) credentials stay in the private
    /// [`NerConfig`].
    pub fn ner_recognizers(&self) -> impl ExactSizeIterator<Item = RegisteredRecognizer<'_>> {
        self.ner.recognizers.iter().map(Into::into)
    }

    /// Every LLM recognizer this engine has registered, in
    /// configuration order.
    ///
    /// Same shape as [`ner_recognizers`], for the LLM lineup.
    ///
    /// [`ner_recognizers`]: Self::ner_recognizers
    pub fn llm_recognizers(&self) -> impl ExactSizeIterator<Item = RegisteredRecognizer<'_>> {
        self.llm.recognizers.iter().map(Into::into)
    }

    /// Analyze one document into an [`Audit`].
    ///
    /// Decodes `document`, drives [`Orchestrator::analyze`], and
    /// projects the report onto the caller-facing [`Audit`].
    /// Captures the body group *and* every container part group
    /// (DOCX embedded images, archive members, ...) the
    /// orchestrator returned; each returned group carries its own
    /// modality tag via its [`EntityGroup`] variant.
    ///
    /// `policies` contributes the label catalog (each
    /// [`PolicyDefinition::labels`] unions in) that drives
    /// recognizer dispatch. Every policy carries its own
    /// [`LabelGroup`]s in [`PolicyDefinition::groups`]; the
    /// engine stamps a `group:<policy_id>:<name>` synthetic tag
    /// on every listed label at request-compile time and rejects
    /// any [`LabelInGroup`] reference the enclosing policy
    /// doesn't declare.
    ///
    /// The catalog is not persisted onto the returned [`Audit`] —
    /// the anonymize path re-derives it from the policy set it
    /// was handed. Pass the same policy set to
    /// [`Self::anonymize`] so its rule bodies match against a
    /// catalog they helped define.
    ///
    /// [`Orchestrator::analyze`]: elide::Orchestrator::analyze
    /// [`EntityGroup`]: crate::entity::EntityGroup
    /// [`LabelGroup`]: nvisy_schema::policy::LabelGroup
    /// [`LabelInGroup`]: nvisy_schema::policy::predicate::Predicate::LabelInGroup
    /// [`PolicyDefinition::labels`]: nvisy_schema::policy::PolicyDefinition::labels
    /// [`PolicyDefinition::groups`]: nvisy_schema::policy::PolicyDefinition::groups
    pub async fn analyze(
        &self,
        document: Document,
        policies: &[PolicyDefinition],
        spec: &AnalyzerParams,
    ) -> Result<Audit> {
        let correlation_id = document.correlation_id;
        let extension = document.extension.clone();
        let mut handle = self.decode(document).await?;
        let (orchestrator, context) =
            self.build_analyze_orchestrator(spec, policies, correlation_id)?;
        let directives = build_analyze_directives(spec);
        let mut report = orchestrator.analyze(&mut handle, &directives).await?;

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
            Error::new(
                ErrorKind::CapabilityUnavailable,
                format!(
                    "codec resolved {extension:?} to a modality the orchestrator \
                     has no pipeline for"
                ),
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

        Ok(Audit {
            body: Some(body_group),
            parts,
            context,
        })
    }

    /// Anonymize one document against a policy set and reviewer
    /// overrides.
    ///
    /// Re-decodes `document`, rebuilds a multi-group [`Report`]
    /// from `audit`'s body + parts, drives
    /// [`Orchestrator::anonymize_with`] with the reviewer
    /// overrides extracted from every group and the caller-filtered
    /// `policies`, merges the redaction events elide stamped back
    /// onto each `EntityRecord`'s provenance chain, and returns
    /// the re-encoded [`Document`].
    ///
    /// `audit` is taken by `&mut`: after redaction, each entity's
    /// `provenance.events` gains one redaction event per operator
    /// that fired. Callers walk `audit.body`/`audit.parts`
    /// entities' provenance to see who redacted what, under which
    /// rule, and when.
    ///
    /// `policies` is the policy set already filtered by
    /// [`PolicyDefinition::when`] against the per-doc facts; the
    /// engine does not re-evaluate predicates. Each policy
    /// carries its own [`LabelGroup`]s inline via
    /// [`PolicyDefinition::groups`] — same shape as
    /// [`Self::analyze`]. The label catalog is re-derived from
    /// `policies` on every call — policies are the sole source
    /// of label vocabulary. The asserted scope (languages,
    /// jurisdictions, metadata) travels on [`Audit::context`]
    /// from analyze. The document's `correlation_id` is threaded
    /// into tracing spans on the redaction path.
    ///
    /// [`LabelGroup`]: nvisy_schema::policy::LabelGroup
    /// [`PolicyDefinition::groups`]: nvisy_schema::policy::PolicyDefinition::groups
    ///
    /// The body's modality (read from `audit.body`'s
    /// [`EntityGroup`] variant) pins which typed handle the
    /// post-apply re-encode goes through: a container's body
    /// modality regardless of how many other modalities its parts
    /// ride on.
    ///
    /// [`Orchestrator::anonymize_with`]: elide::Orchestrator::anonymize_with
    /// [`PolicyDefinition::when`]: nvisy_schema::policy::PolicyDefinition::when
    /// [`EntityGroup`]: crate::entity::EntityGroup
    pub async fn anonymize(
        &self,
        document: Document,
        policies: &[PolicyDefinition],
        audit: &mut Audit,
    ) -> Result<Document> {
        let body_group = audit.body.as_ref().ok_or_else(|| {
            Error::new(
                ErrorKind::Configuration,
                "anonymize: body group is missing — analyze must run first",
            )
        })?;
        let correlation_id = document.correlation_id;
        let extension = document.extension.clone();
        let content_type = document.content_type.clone();
        let mut handle = self.decode(document).await?;

        let mut report = body_group.insert_into_body(Report::new());
        let mut overrides: Vec<(Uuid, ModalityRedactions)> = Vec::new();
        body_group.collect_overrides_into(&mut overrides);
        for (id, group) in &audit.parts {
            report = group.insert_as_part(report, id);
            group.collect_overrides_into(&mut overrides);
        }

        let orchestrator = self.build_anonymize_orchestrator(
            &audit.context,
            policies,
            &overrides,
            correlation_id,
        )?;
        let mut mutated = orchestrator.anonymize_with(&mut handle, report).await?;

        let bytes = body_group.encode_redacted_from(handle)?;
        if let Some(body) = audit.body.as_mut() {
            body.merge_body_from(&mut mutated);
        }
        for (id, group) in audit.parts.iter_mut() {
            group.merge_part_from(&mut mutated, id);
        }

        Ok(Document {
            bytes,
            extension,
            content_type,
            correlation_id,
        })
    }

    async fn decode(&self, document: Document) -> Result<UntypedDocumentHandle> {
        let Document {
            bytes, extension, ..
        } = document;
        self.formats
            .decode(bytes, extension.as_str())
            .await
            .map_err(|err| {
                Error::new(
                    ErrorKind::MalformedInput,
                    format!("codec decode failed for extension {extension:?}: {err}"),
                )
            })
    }
}

/// Assemble the per-analyze [`Directives`] from `spec.annotations`,
/// registering each feature-gated modality's regions with the set.
///
/// The orchestrator's run-wide scope stays the default; no
/// per-analysis scope override is used at this layer.
fn build_analyze_directives(spec: &AnalyzerParams) -> Directives {
    let directives = Directives::new().with_annotations::<Text>(spec.annotations.text.clone());
    #[cfg(feature = "internal_tabular")]
    let directives = directives.with_annotations::<Tabular>(spec.annotations.tabular.clone());
    #[cfg(feature = "internal_image")]
    let directives = directives.with_annotations::<Image>(spec.annotations.image.clone());
    #[cfg(feature = "internal_audio")]
    let directives = directives.with_annotations::<Audio>(spec.annotations.audio.clone());
    directives
}

/// Dispatch a part to the take-part helper matching its modality
/// `TypeId`. Feature-gated per modality; a part whose modality is
/// disabled in this build falls through to `None`, and the caller
/// treats it the same as a part the engine doesn't model.
fn take_part_dispatch(report: &mut Report, id: &PartId, type_id: TypeId) -> Option<EntityGroup> {
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
