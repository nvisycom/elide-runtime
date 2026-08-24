//! [`Engine`]: the stateless pipeline over [`elide`].
//!
//! Long-lived state (only two things):
//!
//! - A [`Provider`]: the deployment's configuration, already
//!   built. It owns the [`FormatRegistry`] documents decode
//!   through and the recognizer and enricher lineups, and hands
//!   out an [`Orchestrator`] per request.
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
//! [`Audit::scope`]; the label catalog is derived from
//! `policies` afresh on every call so policies stay the single
//! source of truth for label vocabulary. Callers do not re-pass
//! an [`RequestScope`] to anonymize.
//!
//! Hosts hold the returned [`Audit`] between analyze and
//! anonymize however they see fit: in memory, in a run store, in
//! a reviewer UI's state, and hand it back to
//! [`Engine::anonymize`] with any per-entity reviewer overrides
//! folded in.
//!
//! ## Internal layout
//!
//! Sibling modules:
//!
//! - `crate::entity` owns [`ReviewSet`] (the reviewer decisions
//!   that sit beside elide's report) and the projection onto the
//!   overrides a provider applies.
//! - `audit` defines [`Audit`], [`RequestScope`], the analyze →
//!   anonymize bridge; all re-exported at the crate root.
//!
//! [`Audit`]: crate::Audit
//! [`RequestScope`]: crate::RequestScope
//! [`ReviewSet`]: crate::entity::ReviewSet
//! [`FormatRegistry`]: elide::codec::FormatRegistry
//! [`DocumentHandle`]: elide::codec::DocumentHandle
//! [`Orchestrator`]: elide::Orchestrator
//! [`Report`]: elide::Report

mod audit;
#[cfg(feature = "audit-csv")]
mod audit_csv;
mod registered;

use std::mem;

use bytes::Bytes;
use elide::codec::{FormatRegistry, UntypedDocumentHandle};
use elide::modality::Modality;
use elide::modality::audio::Audio;
use elide::modality::image::Image;
use elide::modality::tabular::Tabular;
use elide::modality::text::Text;
use elide::primitive::RasterMode;
use elide::recognition::UsageReport;
use elide::{Directives, Error, ErrorKind, Report, Result};
use elide_governance::PolicyDefinition;
use elide_provider::{Provider, RequestContext, RequestScope};
use serde::Deserialize;

pub use self::audit::Audit;
pub use self::registered::{RegisteredComponents, RegisteredEnricher, RegisteredRecognizer};
use crate::entity::ReviewSet;
use crate::file::Document;

/// Cheaply-cloneable pipeline adapter over [`elide`].
///
/// Wraps a [`Provider`] — the codec registry and the deployment's
/// recognizer and enricher lineups — with the two verbs that run
/// documents through it.
#[derive(Clone)]
pub struct Engine {
    provider: Provider,
}

impl Engine {
    /// An engine over `provider`'s configuration.
    ///
    /// A [`Provider`] holds what a deployment decides once; an
    /// engine adds the verbs that run documents through it. One
    /// provider can back many engines, and cloning either is cheap.
    #[must_use]
    pub fn new(provider: Provider) -> Self {
        Self { provider }
    }

    /// The configuration this engine runs on.
    #[must_use]
    pub fn provider(&self) -> &Provider {
        &self.provider
    }

    /// The codec registry the engine decodes documents through.
    pub fn formats(&self) -> &FormatRegistry {
        self.provider.formats()
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
        let orchestrator = self.provider.report_orchestrator();
        let report = orchestrator.deserialize_report(wire.report)?;

        Ok(Audit {
            report,
            reviews: wire.reviews,
            scope: wire.scope,
            usage: wire.usage,
        })
    }

    /// Every recognizer and enricher this engine has registered,
    /// each lineup in configuration order.
    pub fn components(&self) -> RegisteredComponents {
        RegisteredComponents {
            ner: self
                .provider
                .recognizers()
                .ner
                .iter()
                .map(Into::into)
                .collect(),
            llm: self
                .provider
                .recognizers()
                .llm
                .iter()
                .map(Into::into)
                .collect(),
            ocr: self
                .provider
                .enrichers()
                .ocr
                .iter()
                .map(Into::into)
                .collect(),
            stt: self
                .provider
                .enrichers()
                .stt
                .iter()
                .map(Into::into)
                .collect(),
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
        scope: &RequestScope,
    ) -> Result<Audit> {
        let correlation_id = document.correlation_id;
        let extension = document.extension.clone();
        let mut handle = self.decode(document, scope.raster_mode).await?;
        let orchestrator = self
            .provider
            .analyze_orchestrator(scope, policies, correlation_id)?;
        let mut report = orchestrator
            .analyze(&mut handle, &Directives::new())
            .await?;
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
        self.provider
            .record_picks(scope, policies, correlation_id, &mut report);

        Ok(Audit {
            report,
            reviews: ReviewSet::default(),
            scope: scope.clone(),
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
    /// jurisdictions, metadata) travels on [`Audit::scope`]
    /// from analyze. Tracing spans on this path carry the
    /// `correlation_id` of the document being redacted: the id lives
    /// on the document and nowhere else, so there is never a second
    /// copy to disagree with it.
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
    /// resolve their [`KeyProvider`] from `request`. A key belongs
    /// to the caller asking for redaction, not to the process
    /// serving them, so it arrives per request rather than sitting
    /// on the engine: one engine serves many callers, each with its
    /// own. [`RequestContext::default`] supplies none, which is
    /// right when no policy names a keyed operator; a policy that
    /// names one without a key fails here, saying which policy and
    /// which operator.
    ///
    /// [`TextRedaction::Pseudonymize`]: elide_governance::redaction::TextRedaction::Pseudonymize
    /// [`TextRedaction::HmacHash`]: elide_governance::redaction::TextRedaction::HmacHash
    /// [`TextRedaction::Encrypt`]: elide_governance::redaction::TextRedaction::Encrypt
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
        request: &RequestContext,
    ) -> Result<Document> {
        let correlation_id = document.correlation_id;
        let extension = document.extension.clone();
        let content_type = document.content_type.clone();
        let mut handle = self.decode(document, audit.scope.raster_mode).await?;

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

        let orchestrator = self.provider.anonymize_orchestrator(
            &audit.scope,
            policies,
            &audit.overrides(),
            request,
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
            RasterMode::Auto => {
                self.provider
                    .formats()
                    .decode(bytes, extension.as_str())
                    .await
            }
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
    scope: RequestScope,
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
