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
use elide_core::primitive::OcrMode;
use elide_core::{Error, ErrorKind, Result};
use nvisy_schema::file::Document;
use nvisy_schema::plan::AnalyzerParams;
use nvisy_schema::policy::PolicyDefinition;

pub use self::audit::{Audit, AuditContext};
pub use self::registered::RegisteredRecognizer;
use crate::entity::{EntityGroup, OverrideEntry, take_body, take_part};
use crate::provider::llm::LlmConfig;
use crate::provider::ner::NerConfig;
use crate::provider::ocr::OcrBackend;
use crate::provider::stt::SttBackend;

/// Cheaply-cloneable pipeline adapter over [`elide`].
///
/// Bundles the codec registry, the deployment's recognizer and
/// enricher lineups (NER, LLM, OCR, STT), the shared
/// [`KeyProvider`] (for `HmacHash` and `Encrypt`), and the
/// per-request orchestrator constructor.
///
/// [`KeyProvider`]: elide::redaction::operators::KeyProvider
#[derive(Clone)]
pub struct Engine {
    formats: Arc<FormatRegistry>,
    ner: Arc<NerConfig>,
    llm: Arc<LlmConfig>,
    ocr: Option<Arc<OcrBackend>>,
    stt: Option<Arc<SttBackend>>,
    pub(super) key_provider: Option<Arc<dyn KeyProvider>>,
}

impl Engine {
    /// New engine paired with elide's built-in codec set.
    ///
    /// Uses [`FormatRegistry::with_builtin`] plus empty NER, LLM,
    /// OCR, and STT lineups. Callers that want any inference
    /// recognizer or enricher wire it via the corresponding
    /// builder: [`with_ner`], [`with_llm`], [`with_ocr`],
    /// [`with_stt`]. Callers whose policies use `HmacHash` or
    /// `Encrypt` must wire a key provider via
    /// [`with_key_provider`]. The language-detection enricher
    /// always attaches to text with elide's unrestricted lingua
    /// default.
    ///
    /// [`with_key_provider`]: Self::with_key_provider
    /// [`with_llm`]: Self::with_llm
    /// [`with_ner`]: Self::with_ner
    /// [`with_ocr`]: Self::with_ocr
    /// [`with_stt`]: Self::with_stt
    pub fn new() -> Self {
        Self {
            formats: Arc::new(FormatRegistry::with_builtin()),
            ner: Arc::new(NerConfig::default()),
            llm: Arc::new(LlmConfig::default()),
            ocr: None,
            stt: None,
            key_provider: None,
        }
    }

    /// Set the deployment's NER lineup.
    ///
    /// Consumed once at setup; every wired recognizer attaches to
    /// every request whose modality matches.
    #[must_use]
    pub fn with_ner(mut self, ner: NerConfig) -> Self {
        self.ner = Arc::new(ner);
        self
    }

    /// Set the deployment's LLM lineup.
    ///
    /// Consumed once at setup; every wired recognizer whose
    /// modality list matches the analyzer's modality attaches to
    /// every request.
    #[must_use]
    pub fn with_llm(mut self, llm: LlmConfig) -> Self {
        self.llm = Arc::new(llm);
        self
    }

    /// Wire the deployment's OCR enricher.
    ///
    /// Attaches to the image-modality analyzer on every request.
    /// Skipped when unset.
    #[must_use]
    pub fn with_ocr(mut self, backend: OcrBackend) -> Self {
        self.ocr = Some(Arc::new(backend));
        self
    }

    /// Wire the deployment's speech-to-text enricher.
    ///
    /// Attaches to the audio-modality analyzer on every request.
    /// Skipped when unset.
    #[must_use]
    pub fn with_stt(mut self, backend: SttBackend) -> Self {
        self.stt = Some(Arc::new(backend));
        self
    }

    /// Set the engine-level cryptographic [`KeyProvider`] the
    /// `HmacHash` and `Encrypt` operators resolve their keys
    /// through.
    ///
    /// One provider backs both operators; per-label keys are the
    /// provider's own responsibility. A policy that names either
    /// operator without a provider wired errors at request compile
    /// time — the audit trail names the policy and operator so a
    /// misconfiguration surfaces at load, not silently.
    ///
    /// [`KeyProvider`]: elide::redaction::operators::KeyProvider
    #[must_use]
    pub fn with_key_provider(mut self, provider: Arc<dyn KeyProvider>) -> Self {
        self.key_provider = Some(provider);
        self
    }

    /// The codec registry the engine decodes documents through.
    pub fn formats(&self) -> &FormatRegistry {
        &self.formats
    }

    /// Every NER recognizer this engine has registered, in
    /// configuration order.
    pub fn ner_recognizers(&self) -> impl ExactSizeIterator<Item = RegisteredRecognizer> {
        self.ner.recognizers.iter().map(Into::into)
    }

    /// Every LLM recognizer this engine has registered, in
    /// configuration order.
    pub fn llm_recognizers(&self) -> impl ExactSizeIterator<Item = RegisteredRecognizer> {
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
    /// [`LabelGroup`]s in [`PolicyDefinition::groups`]; a
    /// [`LabelInGroup`] predicate can only name a group its own
    /// policy declared (strict per-policy scoping, enforced at
    /// request compile).
    ///
    /// **Policy precedence.** Policies are evaluated in
    /// submission order. Within a policy, rules are tried in the
    /// order they appear; the first that matches an entity in
    /// the policy's declared label scope wins. Across policies,
    /// rules attach in the order `policies` was submitted, so
    /// policy A's rule at slot N wins over policy B's rule at
    /// slot N+1 for any entity in both policies' scope. Policy
    /// fallbacks fire after every policy's rules have had a
    /// shot (two-pass attach) so a coarse baseline's fallback
    /// does not shadow a subsequent policy's more specific rule.
    ///
    /// **Overlap clustering.** Elide clusters overlapping
    /// detections and picks a single winner across the whole
    /// request; the winner's attribution is stamped on every
    /// clustered entity's redaction event. Two policies whose
    /// entities overlap in the document medium can therefore see
    /// each other's attribution recorded on shared cluster
    /// members. See <https://github.com/nvisycom/elide/issues>
    /// for the per-policy cluster segmentation follow-up.
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
        let mut handle = self.decode(document, spec.ocr_mode).await?;
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
    /// `policies` is the policy set the caller wants applied to
    /// this document. Each policy carries its own [`LabelGroup`]s
    /// inline via [`PolicyDefinition::groups`] — same shape as
    /// [`Self::analyze`]. The label catalog is re-derived from
    /// `policies` on every call — policies are the sole source
    /// of label vocabulary. The asserted scope (languages,
    /// jurisdictions, metadata) travels on [`Audit::context`]
    /// from analyze. The document's `correlation_id` is threaded
    /// into tracing spans on the redaction path.
    ///
    /// **Composition semantics.** Rules attach in submission
    /// order; first match wins across the whole policy set.
    /// Every predicate filters by the enclosing policy's
    /// declared label set — a rule inside policy A cannot fire
    /// on labels only policy B declared. Policy fallbacks attach
    /// after every policy's rules so a coarse baseline's
    /// fallback doesn't shadow subsequent more-specific rules.
    ///
    /// **Reviewer overrides** attach before any policy rule and
    /// carry the overriding policy's authority on the audit event.
    ///
    /// **`Pseudonymize` and keyed operators** —
    /// [`TextRedaction::Pseudonymize`] draws from a per-policy
    /// in-memory vault: the same policy pseudonymising the same
    /// entity twice in a document resolves to the same surrogate,
    /// but two different policies pseudonymising the same entity
    /// draw independent surrogates.
    /// [`TextRedaction::HmacHash`] and [`TextRedaction::Encrypt`]
    /// resolve their [`KeyProvider`] through the engine-level
    /// provider set via [`Engine::with_key_provider`].
    ///
    /// [`TextRedaction::Pseudonymize`]: nvisy_schema::policy::redaction::TextRedaction::Pseudonymize
    /// [`TextRedaction::HmacHash`]: nvisy_schema::policy::redaction::TextRedaction::HmacHash
    /// [`TextRedaction::Encrypt`]: nvisy_schema::policy::redaction::TextRedaction::Encrypt
    /// [`Engine::with_key_provider`]: Self::with_key_provider
    /// [`KeyProvider`]: elide::redaction::operators::KeyProvider
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
        let mut handle = self.decode(document, audit.context.ocr_mode).await?;

        let mut report = body_group.insert_into_body(Report::new());
        let mut overrides: Vec<OverrideEntry> = Vec::new();
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

    async fn decode(&self, document: Document, ocr_mode: OcrMode) -> Result<UntypedDocumentHandle> {
        let Document {
            bytes, extension, ..
        } = document;
        let result = match ocr_mode {
            OcrMode::Auto => self.formats.decode(bytes, extension.as_str()).await,
            _ => {
                self.decode_with_ocr_mode(bytes, extension.as_str(), ocr_mode)
                    .await
            }
        };
        result.map_err(|err| {
            Error::new(
                ErrorKind::MalformedInput,
                format!("codec decode failed for extension {extension:?}: {err}"),
            )
        })
    }

    /// Slow-path decode for requests overriding the default OCR
    /// mode. Rebuilds a [`FormatRegistry`] with the PDF handler
    /// replaced by one wired to `ocr_mode`; other codecs come
    /// from the built-in set. Callers pay one registry build per
    /// non-default request — trivial next to the OCR render
    /// itself (`Force { dpi }` pages the whole document at the
    /// chosen DPI), but not free, so the default path skips it.
    #[cfg(feature = "codec-pdf-render")]
    async fn decode_with_ocr_mode(
        &self,
        bytes: bytes::Bytes,
        extension: &str,
        ocr_mode: OcrMode,
    ) -> std::result::Result<UntypedDocumentHandle, elide_core::Error> {
        use elide::codec::handler::pdf_format_with;
        let registry =
            FormatRegistry::with_builtin().with_replaced_format(pdf_format_with(ocr_mode));
        registry.decode(bytes, extension).await
    }

    /// Fallback when the render feature is off: any non-default
    /// mode falls through to the shared registry.
    #[cfg(not(feature = "codec-pdf-render"))]
    async fn decode_with_ocr_mode(
        &self,
        bytes: bytes::Bytes,
        extension: &str,
        _ocr_mode: OcrMode,
    ) -> std::result::Result<UntypedDocumentHandle, elide_core::Error> {
        self.formats.decode(bytes, extension).await
    }
}

impl Default for Engine {
    fn default() -> Self {
        Self::new()
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
