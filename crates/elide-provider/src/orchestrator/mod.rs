//! [`Provider`]: a deployment's configuration, ready to build
//! orchestrators from.
//!
//! Configuration is parsed once; an [`Orchestrator`] is built per
//! request, because it carries request data — the policies in
//! force, the caller's scope, a correlation id — that no
//! deployment-wide value could hold. What the provider holds is the
//! half that does not change between requests: the codec registry,
//! the recognizer and enricher lineups, and the key provider.
//!
//! The returned orchestrator borrows the registry from the provider
//! that made it, so a provider outlives every orchestrator it hands
//! out. Hold one for the life of the process.

mod clients;

#[cfg(feature = "google-document-ai")]
pub(crate) use self::clients::Clients;
mod codec;
mod context;
mod key;
mod pass;
mod request;
mod validate;

use std::sync::Arc;

use elide::codec::FormatRegistry;
use elide::{Error, ErrorKind, Orchestrator, Report, Restorer, Result};
use elide_deployment::Deployment;
use elide_review::HiddenContentPlan;
use elide_schema::compile_catalog;
use elide_schema::policy::Policy;
use elide_schema::recognition::Recognition;

pub use self::codec::CodecParams;
pub use self::context::DocumentContext;
pub use self::key::KeyConfig;
pub use self::pass::Pass;
pub use self::request::RequestContext;
use self::validate::validate_scope_references;
use crate::detect::{
    Availability, ComponentSelection, Enrichers, Recognizers, ResolvedComponents, Resolver,
    analyzers,
};
use crate::redact::anonymizers;

/// A deployment's configuration, ready to build orchestrators from.
///
/// Cheap to clone: one [`Arc`] around the whole configuration, so a
/// host hands a clone to each worker rather than rebuilding it, and
/// a clone costs one refcount rather than one per field.
#[derive(Debug, Clone)]
pub struct Provider {
    inner: Arc<ProviderInner>,
}

/// The configuration a [`Provider`] shares between its clones.
///
/// Behind one [`Arc`] rather than an `Arc` per field: these are
/// decided together at startup, read together on every request, and
/// never change independently, so they are one value.
#[derive(Debug)]
pub(crate) struct ProviderInner {
    /// The codec registry documents decode through.
    pub(crate) formats: FormatRegistry,
    /// The recognizer lineups.
    ///
    /// Behind their own [`Arc`] because [`Provider::restricted_to`]
    /// rebuilds this struct per restricted caller while the lineups
    /// themselves never change: sharing them by refcount keeps that
    /// from copying every configured component each time.
    pub(crate) recognizers: Arc<Recognizers>,
    /// The enricher lineups, shared for the same reason as
    /// [`recognizers`](Self::recognizers).
    pub(crate) enrichers: Arc<Enrichers>,
    /// Clients that had to be built before any request arrived.
    ///
    /// Empty unless the deployment names a backend whose client
    /// cannot be constructed in a synchronous per-request compile;
    /// see [`clients`](mod@clients).
    pub(crate) clients: Arc<clients::Clients>,
    /// Which of those components a caller through this provider
    /// may run. Fixed here rather than per request: it is a
    /// property of who is calling, not of the document.
    pub(crate) availability: Availability,
}

impl Provider {
    /// Build the provider a [`Deployment`] describes.
    ///
    /// The usual path: a host loads a deployment — from TOML, from
    /// its own database, or built in code — and this turns it into
    /// the backends it names. The availability the deployment
    /// declares comes along, so an operator's restriction is stated
    /// where the deployment is described rather than applied as a
    /// separate step afterwards.
    /// # Panics
    ///
    /// Panics if `deployment` is not valid. Every constructor in
    /// `elide-deployment` validates, so the only way to hold an
    /// invalid one is to deserialize it through serde directly —
    /// which that crate documents as requiring a `validate` call.
    /// Panicking here is the lesser fault: the alternative is a
    /// provider that builds, serves, and then panics from inside a
    /// request when a limiter rejects a zero rate, far from the
    /// configuration that caused it.
    ///
    /// Use [`try_new`](Self::try_new) to get the error instead.
    #[must_use]
    pub fn new(deployment: Deployment) -> Self {
        if let Err(err) = deployment.validate() {
            panic!(
                "Provider::new was given an invalid deployment: {err}. Deserializing a \
Deployment through serde skips validation, so call `validate` first or use \
`Provider::try_new`."
            );
        }
        let (recognizers, enrichers, availability) = deployment.into_parts();
        Self {
            inner: Arc::new(ProviderInner {
                formats: FormatRegistry::with_builtin(),
                recognizers: Arc::new(recognizers),
                enrichers: Arc::new(enrichers),
                availability,
                clients: Arc::new(clients::Clients::default()),
            }),
        }
    }

    /// Build the provider a [`Deployment`] describes, awaiting any
    /// backend client that cannot be constructed synchronously.
    ///
    /// Use this instead of [`new`](Self::new) when the deployment
    /// may name Google Cloud Document AI, whose client builder is
    /// `async`. For every other deployment it does the same thing
    /// and the `await` returns immediately.
    ///
    /// A deployment naming such a backend and built through
    /// [`new`](Self::new) is refused at request compile rather than
    /// silently skipping the enricher.
    ///
    /// # Errors
    ///
    /// Returns [`Configuration`](ErrorKind::Configuration) if a
    /// client cannot be built.
    pub async fn try_new(deployment: Deployment) -> Result<Self> {
        deployment
            .validate()
            .map_err(|err| Error::new(ErrorKind::Configuration, err.to_string()))?;
        let (recognizers, enrichers, availability) = deployment.into_parts();
        let clients = clients::Clients::build(&enrichers).await?;
        Ok(Self {
            inner: Arc::new(ProviderInner {
                formats: FormatRegistry::with_builtin(),
                recognizers: Arc::new(recognizers),
                enrichers: Arc::new(enrichers),
                availability,
                clients: Arc::new(clients),
            }),
        })
    }

    /// Assemble from already-built lineups, with everything
    /// available.
    ///
    /// For a caller holding the pieces and no deployment to wrap
    /// them in; [`new`](Self::new) is the usual path.
    ///
    /// # Errors
    ///
    /// As [`Deployment::try_from_parts`], which this goes through:
    /// the lineups are validated before they become a deployment.
    pub fn try_from_parts(recognizers: Recognizers, enrichers: Enrichers) -> Result<Self> {
        let deployment = Deployment::try_from_parts(recognizers, enrichers, Availability::All)
            .map_err(|err| Error::new(ErrorKind::Configuration, err.to_string()))?;
        Ok(Self::new(deployment))
    }

    /// The same provider, restricted to the components
    /// `availability` permits.
    ///
    /// Cheap: the lineups and the codec registry are shared behind
    /// an [`Arc`], so a deployment serving several restricted
    /// callers builds one provider and narrows it per caller
    /// rather than rebuilding.
    #[must_use]
    pub fn restricted_to(&self, availability: Availability) -> Self {
        Self {
            inner: Arc::new(ProviderInner {
                formats: self.inner.formats.clone(),
                recognizers: Arc::clone(&self.inner.recognizers),
                enrichers: Arc::clone(&self.inner.enrichers),
                availability,
                clients: Arc::clone(&self.inner.clients),
            }),
        }
    }

    /// Which components a caller through this provider may run.
    #[must_use]
    pub fn availability(&self) -> &Availability {
        &self.inner.availability
    }

    /// The recognizer names `selection` resolves to under this
    /// provider's availability, in configuration order.
    ///
    /// The same resolution a request performs, exposed so a caller
    /// can show what a selection would run — or discover what it
    /// may run at all — without analyzing a document.
    ///
    /// # Errors
    ///
    /// Returns the same refusals a request would: naming a
    /// component this provider withholds, naming one that is not
    /// registered at all, or narrowing every component away.
    pub fn resolved_components(
        &self,
        selection: &ComponentSelection,
    ) -> Result<ResolvedComponents> {
        let availability = &self.inner.availability;
        let recognizers = &self.inner.recognizers;
        let enrichers = &self.inner.enrichers;
        let resolver = Resolver::new(availability, selection);
        resolver.validate_keys(recognizers, enrichers)?;
        let resolved = ResolvedComponents {
            ner: resolver.resolve_names(&recognizers.ner),
            llm: resolver.resolve_names(&recognizers.llm),
            ocr: resolver.resolve_names(&enrichers.ocr),
            stt: resolver.resolve_names(&enrichers.stt),
        };
        resolver.refuse_if_no_recognizer(recognizers, resolved.ner.len() + resolved.llm.len())?;
        Ok(resolved)
    }

    /// The codec registry documents are decoded through.
    #[must_use]
    pub fn formats(&self) -> &FormatRegistry {
        &self.inner.formats
    }

    /// The recognizer lineups this provider was configured with.
    #[must_use]
    pub fn recognizers(&self) -> &Recognizers {
        &self.inner.recognizers
    }

    /// The enricher lineups this provider was configured with.
    #[must_use]
    pub fn enrichers(&self) -> &Enrichers {
        &self.inner.enrichers
    }
}

impl Provider {
    /// Build an [`Orchestrator`] for the analyze path: compile
    /// every per-modality analyzer for the components `selection`
    /// runs, wire empty anonymizers (analyze doesn't run
    /// redaction), and stamp the pass's [`Scope`].
    ///
    /// The label catalog is derived from the pass's policies: every
    /// submitted [`Policy::label_scope`] unions into one
    /// [`LabelCatalog`] used to drive recognizer dispatch and
    /// tag-based selector matching.
    ///
    /// [`Scope`]: elide::recognition::Scope
    /// [`LabelCatalog`]: elide::entity::LabelCatalog
    /// [`Policy::label_scope`]: elide_schema::policy::Policy::label_scope
    pub fn analyze_orchestrator(
        &self,
        pass: &Pass<'_>,
        selection: &ComponentSelection,
    ) -> Result<Orchestrator> {
        let scope = pass.scope(pass.catalog()?);
        Ok(analyzers(&self.inner, pass.recognition, selection)?
            .with_registry(self.inner.formats.clone())
            .with_scope(scope))
    }

    /// Build an [`Orchestrator`] for the anonymize path: reuse
    /// the persisted [`DocumentContext`] from analyze, re-derive the
    /// label catalog from `policies`, wire them onto every
    /// modality's anonymizer, and skip the analyzer compile
    /// (analysis already happened; only [`Anonymizer`](elide::redaction::Anonymizer) state
    /// matters here).
    ///
    /// Only the anonymizer half of each pipeline is built:
    /// recognition already ran at analyze, so [`with_anonymizer`]
    /// defaults the analyzer rather than us constructing four that
    /// never see a document.
    ///
    /// `report` is the reviewed report anonymize will apply: the
    /// hidden-content plan, which flagged items go and which stay, is
    /// worked out from it. `key` is what `HmacHash` and `Encrypt`
    /// resolve through.
    ///
    /// [`with_anonymizer`]: Orchestrator::with_anonymizer
    pub fn anonymize_orchestrator(
        &self,
        pass: &Pass<'_>,
        report: &Report,
        key: Option<&KeyConfig>,
    ) -> Result<Orchestrator> {
        let catalog = pass.catalog()?;
        let plan = HiddenContentPlan::new(report, &catalog);
        let orchestrator = anonymizers(&catalog, pass.policies, &plan, key.map(KeyConfig::build))?;
        Ok(orchestrator
            .with_registry(self.inner.formats.clone())
            .with_scope(pass.scope(catalog)))
    }

    /// The hidden content of `report`, and which of it anonymize
    /// would remove under `policies`.
    ///
    /// The plan anonymize applies, worked out the same way: an item
    /// goes when the catalog `policies` and `recognition` compile to
    /// declares its label, when a finding sits in its value, or when
    /// a reviewer removed it, and stays otherwise. Call it after
    /// analyze for the preview, and again after a review lands to
    /// see what changed.
    ///
    /// # Errors
    ///
    /// Returns [`Configuration`](ErrorKind::Configuration) if the
    /// policies cannot compile a catalog, as analyze would.
    pub fn hidden_content(
        &self,
        report: &Report,
        recognition: &[Recognition],
        policies: &[Policy],
    ) -> Result<HiddenContentPlan> {
        validate_scope_references(policies)?;
        let catalog = compile_catalog(policies, recognition)?;
        Ok(HiddenContentPlan::new(report, &catalog))
    }

    /// A [`Restorer`] knowing every modality this provider handles,
    /// for reading a serialized report, review, or artifact set
    /// back, routing each part to the modality that produced it.
    ///
    /// Needs only the modality registry — no pipelines, no scope,
    /// no policies — so it is not an orchestrator, whose analyzers
    /// and anonymizers would be discarded unused.
    #[must_use]
    pub fn restorer(&self) -> Restorer {
        let mut restorer = Report::restorer();
        elide_schema::each_modality!(|M| restorer = restorer.with_modality::<M>());
        restorer
    }

    /// Record each entity's operator *pick* onto its audit trail,
    /// without applying anything.
    ///
    /// Runs at the end of analyze so the returned audit answers
    /// "what would happen to this entity, and why" before a reviewer
    /// decides anything. Each covered entity gains a [`Selected`]
    /// event naming the operator, the rule that matched it, and the
    /// policy's own rationale.
    ///
    /// Without this a reviewer sees only *that* an entity was
    /// detected, never what is about to happen to it: the pick would
    /// first appear after apply, when the document is already
    /// redacted.
    ///
    /// # Errors
    ///
    /// Returns [`Configuration`](ErrorKind::Configuration) if a
    /// policy declares a scope twice or a rule references a scope it
    /// never declared, and nothing is recorded.
    ///
    /// Also returns the compile error if a policy's operators cannot
    /// be wired (an `HmacHash` with no [`KeyProvider`], say). That
    /// one is informational: a pick only names the operator that
    /// *would* run, so callers on the analyze path may carry on
    /// without one, and the same policy fails loudly again at
    /// anonymize where the operator actually runs.
    ///
    /// [`KeyProvider`]: elide::redaction::operators::KeyProvider
    /// [`Selected`]: elide::entity::audit::AuditKind::Selected
    pub fn record_picks(&self, pass: &Pass<'_>, report: &mut Report) -> Result<()> {
        // Same gate the orchestrator builders run. This method is a
        // public entry point that *writes* to `report`, so skipping
        // it would stamp `Selected` events resolved against the
        // wrong labelset rather than failing: a duplicate scope name
        // makes a `LabelInScope` rule pick one labelset while the
        // catalog unions both, and the reviewer would act on the
        // misleading provenance.
        let catalog = pass.catalog()?;
        // The same anonymizers anonymize builds, minus the key, so
        // the plan a reviewer sees is the plan that runs.
        let plan = HiddenContentPlan::new(report, &catalog);
        anonymizers(&catalog, pass.policies, &plan, None)?
            .with_scope(pass.scope(catalog))
            .pick(report)
    }
}
