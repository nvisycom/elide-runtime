//! [`Engine`]: the stateless pipeline over [`elide`].
//!
//! Its only long-lived state is a [`Provider`]: the deployment's
//! configuration, already built. It owns the [`FormatRegistry`]
//! documents decode through and the recognizer and enricher
//! lineups. [`Engine`] clones cheaply (`Arc` under the hood).
//!
//! ## Per-document verbs
//!
//! - [`Engine::analyze`] decodes raw bytes, runs detection, and
//!   projects the report — body and every container part — onto an
//!   [`Audit`].
//! - [`Engine::anonymize`] decodes those bytes again, layers the
//!   reviewer overrides and policy set onto each modality's
//!   anonymizer, and returns the re-encoded [`Document`].
//!
//! Both build a fresh [`Orchestrator`] per call: a small map of
//! trait objects keyed by modality `TypeId`. Building per call is
//! what lets policies re-resolve per document without mutating
//! shared state.
//!
//! Recognition inputs have two owners. Caller-asserted facts
//! (languages, jurisdictions, tags) travel between the calls on
//! [`Audit::context`], so anonymize compiles against the vocabulary
//! analyze used. The label catalog is derived from `policies`
//! afresh every call, so policies stay the single source of truth
//! for label vocabulary — and stay live between the two passes.
//!
//! Hosts hold the [`Audit`] between the two calls however they see
//! fit — in memory, a run store, a reviewer UI — and hand it back
//! with any reviewer decisions folded in.
//!
//! [`Audit`]: crate::Audit
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
use elide_provider::{CodecParams, DocumentContext, KeyConfig, Provider, RequestContext};
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
        let report = self.provider.deserialize_report(wire.report)?;

        Ok(Audit {
            report,
            reviews: wire.reviews,
            context: wire.context,
            codec: wire.codec,
            usage: wire.usage,
        })
    }

    /// Every recognizer and enricher this engine has registered,
    /// each lineup in configuration order.
    pub fn components(&self) -> RegisteredComponents {
        let recognizers = self.provider.recognizers();
        let enrichers = self.provider.enrichers();
        RegisteredComponents {
            ner: recognizers.ner.iter().map(Into::into).collect(),
            llm: recognizers.llm.iter().map(Into::into).collect(),
            ocr: enrichers.ocr.iter().map(Into::into).collect(),
            stt: enrichers.stt.iter().map(Into::into).collect(),
        }
    }

    /// Analyze one document into an [`Audit`].
    ///
    /// Decodes `document`, drives [`Orchestrator::analyze`], and
    /// returns the report it produced — the body *and* every
    /// container part (DOCX embedded images, archive members, ...)
    /// — with the recognition context and what the pass cost.
    ///
    /// `policies` contributes the label catalog that drives
    /// recognizer dispatch. Each carries its own [`LabelScope`]s,
    /// and a [`LabelInScope`] predicate can only name a group its
    /// own policy declared — enforced at request compile.
    ///
    /// The catalog is not persisted: [`anonymize`](Self::anonymize)
    /// re-derives it from the policy set it is handed. Pass the same
    /// set there, so rule bodies match against a catalog they helped
    /// define. See that method for how rules compose.
    ///
    /// # Overlap clustering
    ///
    /// elide clusters overlapping detections and picks one winner
    /// per cluster, stamping its attribution on every member. Two
    /// policies whose entities overlap can therefore each see the
    /// other's attribution on shared members. Per-policy cluster
    /// segmentation is a follow-up upstream.
    ///
    /// # Errors
    ///
    /// Returns [`Configuration`](ErrorKind::Configuration) for a
    /// policy that cannot compile, and
    /// [`MalformedInput`](ErrorKind::MalformedInput) for a document
    /// the codec cannot decode.
    ///
    /// [`LabelInScope`]: elide_governance::Predicate::LabelInScope
    /// [`LabelScope`]: elide_governance::LabelScope
    /// [`Orchestrator::analyze`]: elide::Orchestrator::analyze
    pub async fn analyze(
        &self,
        document: Document,
        policies: &[PolicyDefinition],
        request: &RequestContext,
    ) -> Result<Audit> {
        let correlation_id = document.correlation_id;
        let extension = document.extension.clone();
        let mut handle = self.decode(document, request.codec).await?;
        let orchestrator =
            self.provider
                .analyze_orchestrator(&request.context, policies, correlation_id)?;
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
        // before a reviewer overrides anything. Purely additive: it
        // appends Selection events and redacts nothing.
        //
        // A failure here does not fail the analyze. Every reason the
        // pick can fail — an unresolvable label, an operator whose
        // key has not arrived yet — is raised again by `anonymize`,
        // which compiles the same policies and does fail. The
        // keyless `HmacHash` case is the common one: the request
        // supplies its key at anonymize, so refusing to analyze
        // would deny the caller detections over a redaction they
        // have not asked for yet, and report the same fault twice.
        //
        // Scope-reference errors never reach here: `analyze_orchestrator`
        // rejects them above, before any of this runs.
        //
        // The observable signal is an audit carrying no `Selection`
        // events.
        let _: Result<()> =
            self.provider
                .record_picks(&request.context, policies, correlation_id, &mut report);

        Ok(Audit {
            report,
            reviews: ReviewSet::default(),
            context: request.context.clone(),
            codec: request.codec,
            usage,
        })
    }

    /// Apply `policies` and the audit's reviewer decisions to
    /// `document`, returning the re-encoded result.
    ///
    /// `audit` is taken by `&mut`: each entity gains a redaction
    /// event per operator that fired, so its provenance records who
    /// redacted what, under which rule.
    ///
    /// The label catalog is re-derived from `policies` on every
    /// call — policies are the sole source of label vocabulary, so
    /// governance stays live between the two passes. What must
    /// *not* drift travels on the audit: the recognition context
    /// and the codec params [`analyze`](Self::analyze) used.
    ///
    /// `key` resolves [`HmacHash`] and [`Encrypt`]. It belongs to
    /// the caller asking for redaction rather than the process
    /// serving them, so it arrives per call and is never recorded
    /// on an audit. `None` is right when no policy names a keyed
    /// operator; a policy that names one without a key fails here.
    ///
    /// # Composition
    ///
    /// Rules attach in submission order, first match wins across
    /// the whole set. Every predicate filters by its own policy's
    /// declared labels, so a rule in policy A cannot fire on labels
    /// only B declared. Policy fallbacks attach after every
    /// policy's rules, so a coarse baseline does not shadow a
    /// later, more specific one. Reviewer overrides attach ahead of
    /// all of them, carrying the overriding policy's authority.
    ///
    /// [`Pseudonymize`] draws from a per-policy vault: one policy
    /// pseudonymising an entity twice gets one surrogate, two
    /// policies get independent ones.
    ///
    /// # Errors
    ///
    /// Returns [`Configuration`](ErrorKind::Configuration) for a
    /// policy that cannot compile or an audit that was never
    /// analyzed, and [`MalformedInput`](ErrorKind::MalformedInput)
    /// for a document the codec cannot decode.
    ///
    /// [`Encrypt`]: elide_governance::redaction::TextRedaction::Encrypt
    /// [`HmacHash`]: elide_governance::redaction::TextRedaction::HmacHash
    /// [`Pseudonymize`]: elide_governance::redaction::TextRedaction::Pseudonymize
    pub async fn anonymize(
        &self,
        document: Document,
        policies: &[PolicyDefinition],
        audit: &mut Audit,
        key: Option<&KeyConfig>,
    ) -> Result<Document> {
        let correlation_id = document.correlation_id;
        let extension = document.extension.clone();
        let content_type = document.content_type.clone();
        let mut handle = self.decode(document, audit.codec).await?;

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
            &audit.context,
            policies,
            &audit.overrides(),
            key,
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
        codec: CodecParams,
    ) -> Result<UntypedDocumentHandle> {
        let Document {
            bytes, extension, ..
        } = document;
        let result = match codec.raster_mode {
            RasterMode::Auto => {
                self.provider
                    .formats()
                    .decode(bytes, extension.as_str())
                    .await
            }
            _ => {
                self.decode_with_raster_mode(bytes, extension.as_str(), codec.raster_mode)
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
    context: DocumentContext,
    // No `default`: unlike `reviews`/`usage` this is always
    // serialized, so a payload omitting it is malformed rather
    // than empty. Defaulting it to `RasterMode::Auto` would let a
    // document re-decode differently than analyze decoded it, and
    // the entity offsets recorded against the first decode would
    // land on different content.
    codec: CodecParams,
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
