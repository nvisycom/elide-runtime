//! [`Engine`]: the stateless pipeline over [`elide`].
//!
//! Long-lived state (only two things):
//!
//! - The [`FormatRegistry`] over elide's codec set. Decodes raw
//!   bytes into a modality-typed [`DocumentHandle`] at analyze +
//!   anonymize time.
//! - The deployment's NER + LLM lineups (see [`crate::recognition::backend::ner`]
//!   and [`crate::recognition::backend::llm`]). Consulted by the analyzer
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
//! anonymize however they see fit: in memory, in a run store, in
//! a reviewer UI's state, and hand it back to
//! [`Engine::anonymize`] with any per-entity reviewer overrides
//! folded in.
//!
//! ## Internal layout
//!
//! Sibling crate-level modules provide the modality-shaped
//! plumbing:
//!
//! - `crate::recognition` and `crate::redaction` compile
//!   per-modality `spec` and `policies` into per-modality elide
//!   types.
//! - `crate::entity` owns [`ReviewSet`] (the reviewer decisions
//!   that sit beside elide's report) and the redaction override
//!   they compile into.
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
//! [`ReviewSet`]: crate::entity::ReviewSet
//! [`FormatRegistry`]: elide::codec::FormatRegistry
//! [`DocumentHandle`]: elide::codec::DocumentHandle
//! [`Orchestrator`]: elide::Orchestrator
//! [`Report`]: elide::Report

mod audit;
#[cfg(feature = "audit-csv")]
mod audit_csv;
mod orchestrator;
mod registered;

use std::mem;
use std::sync::Arc;

use bytes::Bytes;
use elide::codec::{FormatRegistry, UntypedDocumentHandle};
use elide::modality::Modality;
use elide::modality::audio::Audio;
use elide::modality::image::Image;
use elide::modality::tabular::Tabular;
use elide::modality::text::Text;
use elide::primitive::RasterMode;
use elide::recognition::UsageReport;
use elide::redaction::operators::KeyProvider;
use elide::{Directives, Error, ErrorKind, Report, Result};
use elide_governance::PolicyDefinition;
use serde::Deserialize;

pub use self::audit::{Audit, AuditContext};
pub use self::registered::{RegisteredComponents, RegisteredEnricher, RegisteredRecognizer};
use crate::entity::ReviewSet;
use crate::file::Document;
use crate::plan::AnalyzerParams;
use crate::recognition::{LlmConfig, NerConfig, OcrConfig, SttConfig};

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
    ocr: Arc<OcrConfig>,
    stt: Arc<SttConfig>,
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
            ocr: Arc::new(OcrConfig::default()),
            stt: Arc::new(SttConfig::default()),
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

    /// Set the deployment's OCR enricher lineup.
    ///
    /// The enricher attaches to the image-modality analyzer on
    /// every request. Only one enricher attaches per analyzer
    /// today; the request compile rejects `enrichers.len() > 1`
    /// with a Configuration error. An empty lineup skips the
    /// enricher attach.
    #[must_use]
    pub fn with_ocr(mut self, ocr: OcrConfig) -> Self {
        self.ocr = Arc::new(ocr);
        self
    }

    /// Set the deployment's STT enricher lineup.
    ///
    /// The enricher attaches to the audio-modality analyzer on
    /// every request. Only one enricher attaches per analyzer
    /// today; the request compile rejects `enrichers.len() > 1`
    /// with a Configuration error. An empty lineup skips the
    /// enricher attach.
    #[must_use]
    pub fn with_stt(mut self, stt: SttConfig) -> Self {
        self.stt = Arc::new(stt);
        self
    }

    /// Set the engine-level cryptographic [`KeyProvider`] the
    /// `HmacHash` and `Encrypt` operators resolve their keys
    /// through.
    ///
    /// One provider backs both operators; per-label keys are the
    /// provider's own responsibility. A policy that names either
    /// operator without a provider wired errors at request compile
    /// time: the audit trail names the policy and operator so a
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

    /// Read an [`Audit`] back from its serialized form.
    ///
    /// The counterpart to serializing one: a host persists an audit
    /// between analyze and anonymize, or ships it to a reviewer and
    /// takes it back, and this is how it returns.
    ///
    /// [`Audit`] is [`Serialize`](serde::Serialize) but not
    /// `Deserialize`. A serialized report tags each entity group
    /// with its modality *name*, not its concrete type, and
    /// deserialization cannot be object-safe — so rebuilding one
    /// needs a name-to-type registry. This engine is that registry:
    /// it knows which modalities it handles. A free `from_str` would
    /// need a global one, which would close the door on modalities
    /// elide does not ship.
    ///
    /// # Errors
    ///
    /// Returns [`MalformedInput`](ErrorKind::MalformedInput) if the
    /// payload is not a well-formed audit, or if its report names a
    /// modality this engine has no pipeline for — rebuilding it
    /// would silently drop those entities along with any reviewer
    /// decisions on them.
    pub fn deserialize_audit<'de, D>(&self, deserializer: D) -> Result<Audit>
    where
        D: serde::Deserializer<'de>,
    {
        let wire = AuditWire::deserialize(deserializer)
            .map_err(|err| Error::new(ErrorKind::MalformedInput, err.to_string()))?;

        // The report is the one part this engine must rebuild
        // itself; everything else on an audit is plain data.
        let orchestrator = self.build_report_orchestrator();
        let report = orchestrator.deserialize_report(wire.report)?;

        Ok(Audit {
            report,
            reviews: wire.reviews,
            context: wire.context,
            usage: wire.usage,
        })
    }

    /// Every recognizer and enricher this engine has registered,
    /// each lineup in configuration order.
    pub fn components(&self) -> RegisteredComponents {
        RegisteredComponents {
            ner: self.ner.recognizers.iter().map(Into::into).collect(),
            llm: self.llm.recognizers.iter().map(Into::into).collect(),
            ocr: self.ocr.enrichers.iter().map(Into::into).collect(),
            stt: self.stt.enrichers.iter().map(Into::into).collect(),
        }
    }

    /// Analyze one document into an [`Audit`].
    ///
    /// Decodes `document`, drives [`Orchestrator::analyze`], and
    /// returns the report it produced — the body *and* every
    /// container part (DOCX embedded images, archive members, ...)
    /// — wrapped in an [`Audit`] with the recognition context and
    /// what the pass cost.
    ///
    /// `policies` contributes the label catalog (each
    /// [`PolicyDefinition::label_scope`] unions in) that drives
    /// recognizer dispatch. Every policy carries its own
    /// [`LabelScope`]s in [`PolicyDefinition::scopes`]; a
    /// [`LabelInScope`] predicate can only name a group its own
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
    /// The catalog is not persisted onto the returned [`Audit`] -
    /// the anonymize path re-derives it from the policy set it
    /// was handed. Pass the same policy set to
    /// [`Self::anonymize`] so its rule bodies match against a
    /// catalog they helped define.
    ///
    /// [`Orchestrator::analyze`]: elide::Orchestrator::analyze
    /// [`LabelScope`]: elide_governance::LabelScope
    /// [`LabelInScope`]: elide_governance::Predicate::LabelInScope
    /// [`PolicyDefinition::label_scope`]: elide_governance::PolicyDefinition::label_scope
    /// [`PolicyDefinition::scopes`]: elide_governance::PolicyDefinition::scopes
    pub async fn analyze(
        &self,
        document: Document,
        policies: &[PolicyDefinition],
        spec: &AnalyzerParams,
    ) -> Result<Audit> {
        let correlation_id = document.correlation_id;
        let extension = document.extension.clone();
        let mut handle = self.decode(document, spec.raster_mode).await?;
        let (orchestrator, context) =
            self.build_analyze_orchestrator(spec, policies, correlation_id)?;
        let directives = build_analyze_directives(spec);
        let mut report = orchestrator.analyze(&mut handle, &directives).await?;
        // Cloned off the report: elide derives usage during
        // analysis and drops it when a report is rebuilt from the
        // wire, so the audit carries its own copy.
        let usage = report.usage().clone();

        // A document whose codec resolved to a modality with no
        // registered pipeline produces no body, and nothing
        // downstream can act on it. Probing each modality in turn
        // is how the body's own modality is discovered: a report
        // exposes its parts' modalities but not its body's.
        if !has_body(&report) {
            return Err(Error::new(
                ErrorKind::CapabilityUnavailable,
                format!(
                    "codec resolved {extension:?} to a modality the orchestrator \
                     has no pipeline for"
                ),
            ));
        }

        // Record what each entity's policy pick would be, so the
        // returned audit answers "what happens to this, and why"
        // before a reviewer overrides anything. Purely additive:
        // it appends Selection events and redacts nothing.
        self.record_picks(&context, policies, correlation_id, &mut report);

        Ok(Audit {
            report,
            reviews: ReviewSet::default(),
            context,
            usage,
        })
    }

    /// Anonymize one document against a policy set and reviewer
    /// overrides.
    ///
    /// Re-decodes `document`, rebuilds a multi-group [`Report`]
    /// Drives [`Orchestrator::anonymize_with`] with the audit's own
    /// report, the reviewer decisions compiled into per-entity
    /// overrides, and the caller-filtered `policies`, then returns
    /// the re-encoded [`Document`].
    ///
    /// `audit` is taken by `&mut`: after redaction, each entity's
    /// `provenance.events` gains one redaction event per operator
    /// that fired. Callers walk `audit.body`/`audit.parts`
    /// entities' provenance to see who redacted what, under which
    /// rule, and when.
    ///
    /// `policies` is the policy set the caller wants applied to
    /// this document. Each policy carries its own [`LabelScope`]s
    /// inline via [`PolicyDefinition::scopes`]: same shape as
    /// [`Self::analyze`]. The label catalog is re-derived from
    /// `policies` on every call: policies are the sole source
    /// of label vocabulary. The asserted scope (languages,
    /// jurisdictions, metadata) travels on [`Audit::context`]
    /// from analyze. The document's `correlation_id` is threaded
    /// into tracing spans on the redaction path.
    ///
    /// **Composition semantics.** Rules attach in submission
    /// order; first match wins across the whole policy set.
    /// Every predicate filters by the enclosing policy's
    /// declared label set: a rule inside policy A cannot fire
    /// on labels only policy B declared. Policy fallbacks attach
    /// after every policy's rules so a coarse baseline's
    /// fallback doesn't shadow subsequent more-specific rules.
    ///
    /// **Reviewer overrides** attach before any policy rule and
    /// carry the overriding policy's authority on the audit event.
    ///
    /// **`Pseudonymize` and keyed operators** -
    /// [`TextRedaction::Pseudonymize`] draws from a per-policy
    /// in-memory vault: the same policy pseudonymising the same
    /// entity twice in a document resolves to the same surrogate,
    /// but two different policies pseudonymising the same entity
    /// draw independent surrogates.
    /// [`TextRedaction::HmacHash`] and [`TextRedaction::Encrypt`]
    /// resolve their [`KeyProvider`] through the engine-level
    /// provider set via [`Engine::with_key_provider`].
    ///
    /// [`TextRedaction::Pseudonymize`]: elide_governance::redaction::TextRedaction::Pseudonymize
    /// [`TextRedaction::HmacHash`]: elide_governance::redaction::TextRedaction::HmacHash
    /// [`TextRedaction::Encrypt`]: elide_governance::redaction::TextRedaction::Encrypt
    /// [`Engine::with_key_provider`]: Self::with_key_provider
    /// [`KeyProvider`]: elide::redaction::operators::KeyProvider
    ///
    /// [`LabelScope`]: elide_governance::LabelScope
    /// [`PolicyDefinition::scopes`]: elide_governance::PolicyDefinition::scopes
    ///
    /// The body's own modality pins which typed handle the
    /// post-apply re-encode goes through: a container re-encodes
    /// through its body's modality regardless of how many others
    /// its parts ride on.
    ///
    /// [`Orchestrator::anonymize_with`]: elide::Orchestrator::anonymize_with
    pub async fn anonymize(
        &self,
        document: Document,
        policies: &[PolicyDefinition],
        audit: &mut Audit,
    ) -> Result<Document> {
        let correlation_id = document.correlation_id;
        let extension = document.extension.clone();
        let content_type = document.content_type.clone();
        let mut handle = self.decode(document, audit.context.raster_mode).await?;

        // An audit with no body was never analyzed. Applying it
        // would return the document unredacted and report success,
        // which reads exactly like "nothing to redact" — so refuse
        // instead of handing back bytes a caller may believe are
        // clean.
        if !has_body(&audit.report) {
            return Err(Error::new(
                ErrorKind::Configuration,
                "anonymize: the audit has no body; analyze must run first",
            ));
        }

        // Materialise pending suppressions onto their entities:
        // elide reads the trail, not our review set, to decide what
        // the redaction pass skips.
        audit.apply_suppressions();

        let orchestrator = self.build_anonymize_orchestrator(
            &audit.context,
            policies,
            &audit.reviews,
            correlation_id,
        )?;

        // The report moves through apply and comes back mutated,
        // every entity carrying the redaction event elide stamped.
        // Swapped out and back so the caller's audit ends up holding
        // the applied report rather than the pre-apply one.
        let report = mem::replace(&mut audit.report, Report::new());
        audit.report = orchestrator.anonymize_with(&mut handle, report).await?;

        let bytes = encode_redacted(handle)?;

        Ok(Document {
            bytes,
            extension,
            content_type,
            correlation_id,
        })
    }

    async fn decode(
        &self,
        document: Document,
        raster_mode: RasterMode,
    ) -> Result<UntypedDocumentHandle> {
        let Document {
            bytes, extension, ..
        } = document;
        let result = match raster_mode {
            RasterMode::Auto => self.formats.decode(bytes, extension.as_str()).await,
            _ => {
                self.decode_with_raster_mode(bytes, extension.as_str(), raster_mode)
                    .await
            }
        };
        result.map_err(|err| {
            // A missing renderer is not malformed input; keep the kind so
            // callers can tell "unsupported build" from "bad document".
            let kind = match err.kind() {
                ErrorKind::CapabilityUnavailable => ErrorKind::CapabilityUnavailable,
                _ => ErrorKind::MalformedInput,
            };
            Error::new(
                kind,
                format!("codec decode failed for extension {extension:?}: {err}"),
            )
        })
    }

    /// Slow-path decode for requests overriding the default raster
    /// mode. Rebuilds a [`FormatRegistry`] with the PDF handler
    /// replaced by one wired to `raster_mode`; other codecs come
    /// from the built-in set. Callers pay one registry build per
    /// non-default request: trivial next to the render itself
    /// (`Always { dpi }` pages the whole document at the chosen
    /// DPI), but not free, so the default path skips it.
    #[cfg(feature = "codec-pdf-render")]
    async fn decode_with_raster_mode(
        &self,
        bytes: bytes::Bytes,
        extension: &str,
        raster_mode: RasterMode,
    ) -> std::result::Result<UntypedDocumentHandle, elide::Error> {
        use elide::codec::handler::pdf_format_with;
        let registry =
            FormatRegistry::with_builtin().with_replaced_format(pdf_format_with(raster_mode));
        registry.decode(bytes, extension).await
    }

    /// Fallback when the render feature is off. Only non-default
    /// modes reach here, and none of them can be honoured without
    /// the renderer, so refuse rather than decode with the shared
    /// registry: silently substituting `Auto` would hand back a
    /// text-layer extraction to a caller who asked to rasterize.
    #[cfg(not(feature = "codec-pdf-render"))]
    async fn decode_with_raster_mode(
        &self,
        _bytes: bytes::Bytes,
        _extension: &str,
        raster_mode: RasterMode,
    ) -> std::result::Result<UntypedDocumentHandle, elide::Error> {
        Err(elide::Error::new(
            elide::ErrorKind::CapabilityUnavailable,
            format!(
                "raster mode {raster_mode:?} requires the `codec-pdf-render` feature; \
                 rebuild with it enabled or request the default mode"
            ),
        ))
    }
}

/// The wire shape of an [`Audit`], with the report left as raw
/// values.
///
/// Mirrors `Audit`'s field names so one serialized form reads back
/// through both. The report cannot be deserialized here — it needs
/// the engine's modality registry — so it is buffered and handed to
/// [`Orchestrator::deserialize_report`] afterwards.
///
/// [`Orchestrator::deserialize_report`]: elide::Orchestrator::deserialize_report
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct AuditWire {
    report: serde_value::Value,
    #[serde(default)]
    reviews: ReviewSet,
    context: AuditContext,
    #[serde(default)]
    usage: UsageReport,
}

/// Whether `report` has a body at all.
///
/// A report exposes its parts' modalities but not its body's, so
/// the body is discovered by probing each modality in turn: at most
/// one matches. Analyze uses this to reject a document whose codec
/// resolved to a modality with no registered pipeline; anonymize
/// uses it to reject an audit that never went through analyze.
fn has_body(report: &Report) -> bool {
    report.entities::<Text>().is_some()
        || report.entities::<Tabular>().is_some()
        || report.entities::<Image>().is_some()
        || report.entities::<Audio>().is_some()
}

/// Re-encode the redacted handle back into document bytes.
///
/// [`DocumentHandle::encode`] is per-modality, so the untyped
/// handle has to be converted to the typed one first. Which
/// modality that is, is the handle's own answer: it is asked
/// directly rather than inferred from the report, so a container
/// whose parts span modalities still re-encodes through its body's.
///
/// [`DocumentHandle::encode`]: elide::codec::DocumentHandle::encode
fn encode_redacted(handle: UntypedDocumentHandle) -> Result<Bytes> {
    /// Take the first modality the handle claims to be, and encode
    /// through it. Listing them here keeps the modality set in one
    /// place rather than a nest of fallible conversions.
    macro_rules! encode_first_match {
        ($handle:expr, $($modality:ty),+ $(,)?) => {{
            let handle = $handle;
            $(
                if handle.is::<$modality>() {
                    // The `is` check just passed, so the conversion
                    // cannot fail.
                    let Ok(typed) = handle.into::<$modality>() else {
                        unreachable!("handle reports itself as this modality")
                    };
                    return encode_typed(typed);
                }
            )+
            handle
        }};
    }

    let _handle = encode_first_match!(handle, Text, Tabular, Image, Audio);
    Err(Error::new(
        ErrorKind::Redaction,
        "post-apply re-encode: the document handle is a modality this \
         engine has no codec for",
    ))
}

/// Encode one typed handle, mapping the codec's error into a
/// redaction-stage one so a failure here is not mistaken for a
/// malformed input.
fn encode_typed<M: Modality>(typed: elide::codec::DocumentHandle<M>) -> Result<Bytes>
where
    elide::codec::DocumentHandle<M>: elide::modality::DataWriter<M>,
{
    let content = typed.encode().map_err(|err| {
        Error::new(
            ErrorKind::Redaction,
            format!("post-apply encode failed: {err}"),
        )
    })?;
    Ok(content.into_bytes())
}

impl Default for Engine {
    fn default() -> Self {
        Self::new()
    }
}

/// Assemble the per-analyze [`Directives`] from `spec.annotations`,
/// registering every modality's regions with the set.
///
/// The orchestrator's run-wide scope stays the default; no
/// per-analysis scope override is used at this layer.
fn build_analyze_directives(spec: &AnalyzerParams) -> Directives {
    Directives::new()
        .with_annotations::<Text>(spec.annotations.text.clone())
        .with_annotations::<Tabular>(spec.annotations.tabular.clone())
        .with_annotations::<Image>(spec.annotations.image.clone())
        .with_annotations::<Audio>(spec.annotations.audio.clone())
}
