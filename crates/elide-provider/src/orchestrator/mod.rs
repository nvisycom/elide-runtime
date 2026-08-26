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

mod codec;
mod config;
mod context;
mod key;
mod request;

use std::collections::HashSet;
use std::sync::Arc;

use elide::codec::{FormatRegistry, PartId};
use elide::entity::LabelCatalog;
use elide::modality::audio::Audio;
use elide::modality::image::Image;
use elide::modality::tabular::Tabular;
use elide::modality::text::Text;
use elide::recognition::Scope;
use elide::redaction::Anonymizer;
use elide::{Error, ErrorKind, Orchestrator, Report, Result};
use elide_governance::modality::RedactableModality;
use elide_governance::{PolicyDefinition, PolicyRule, Predicate, compile_catalog};
use uuid::Uuid;

pub use self::codec::CodecParams;
pub use self::config::ProviderConfig;
pub use self::context::DocumentContext;
pub use self::key::KeyConfig;
pub use self::request::RequestContext;
use crate::recognition::{Enrichers, Recognizers, analyzers};
use crate::redaction::{Pickers, anonymizers, pickers};

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
    pub(crate) recognizers: Recognizers,
    /// The enricher lineups.
    pub(crate) enrichers: Enrichers,
}

impl Provider {
    /// Assemble from already-built parts.
    ///
    /// Not the usual path: a deployment describes itself with a
    /// [`ProviderConfig`] and builds through it. This exists for a
    /// caller holding the pieces already.
    #[must_use]
    pub fn from_parts(recognizers: Recognizers, enrichers: Enrichers) -> Self {
        Self {
            inner: Arc::new(ProviderInner {
                formats: FormatRegistry::with_builtin(),
                recognizers,
                enrichers,
            }),
        }
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
    /// every per-modality analyzer from `spec`, wire empty
    /// anonymizers (analyze doesn't run redaction), and stamp the
    /// request-scoped [`Scope`].
    ///
    /// The label catalog is derived from `policies`: every
    /// submitted [`PolicyDefinition::label_scope`] unions into one
    /// [`LabelCatalog`] used to drive recognizer dispatch and
    /// tag-based selector matching.
    ///
    /// `correlation_id` tags the orchestrator's tracing spans. It
    /// is passed rather than read off `context`, because it belongs
    /// to the document being processed; persisting it onto the
    /// audit is the caller's job.
    ///
    /// [`Scope`]: elide::recognition::Scope
    /// [`LabelCatalog`]: elide::entity::LabelCatalog
    /// [`PolicyDefinition::label_scope`]: elide_governance::PolicyDefinition::label_scope
    pub fn analyze_orchestrator(
        &self,
        context: &DocumentContext,
        policies: &[PolicyDefinition],
        correlation_id: Uuid,
    ) -> Result<Orchestrator> {
        validate_scope_references(policies)?;
        let catalog = compile_catalog(policies)?;
        let live_scope = build_scope(context, catalog, correlation_id);

        let orchestrator = analyzers(&self.inner.recognizers, &self.inner.enrichers)?;
        Ok(orchestrator
            .with_registry(self.inner.formats.clone())
            .with_scope(live_scope))
    }

    /// Build an [`Orchestrator`] for the anonymize path: reuse
    /// the persisted [`DocumentContext`] from analyze, re-derive the
    /// label catalog from `policies`, wire them onto every
    /// modality's anonymizer, and skip the analyzer compile
    /// (analysis already happened; only [`Anonymizer`] state
    /// matters here).
    ///
    /// Only the anonymizer half of each pipeline is built:
    /// recognition already ran at analyze, so [`with_anonymizer`]
    /// defaults the analyzer rather than us constructing four that
    /// never see a document.
    ///
    /// The scope is tagged with the correlation id `context`
    /// carries, which is the document's own: analyze and anonymize
    /// trace under the same id because they concern the same
    /// document.
    ///
    /// [`with_anonymizer`]: Orchestrator::with_anonymizer
    pub fn anonymize_orchestrator(
        &self,
        context: &DocumentContext,
        policies: &[PolicyDefinition],
        key: Option<&KeyConfig>,
        correlation_id: Uuid,
    ) -> Result<Orchestrator> {
        validate_scope_references(policies)?;
        let catalog = compile_catalog(policies)?;
        let live_scope = build_scope(context, catalog.clone(), correlation_id);

        let orchestrator = anonymizers(&catalog, policies, key.map(KeyConfig::build))?;
        Ok(orchestrator
            .with_registry(self.inner.formats.clone())
            .with_scope(live_scope))
    }

    /// Rebuild a serialized [`Report`], routing each entity group
    /// back to the modality that produced it.
    ///
    /// Needs only the modality registry — no pipelines, no scope,
    /// no policies — so it goes through [`Report::deserializer`]
    /// rather than constructing an orchestrator whose analyzers and
    /// anonymizers would be discarded unused.
    ///
    /// # Errors
    ///
    /// Returns [`MalformedInput`](ErrorKind::MalformedInput) if the
    /// payload is not a well-formed report, or names a modality this
    /// provider has no pipeline for.
    pub fn deserialize_report<'de, D>(&self, deserializer: D) -> Result<Report>
    where
        D: serde::Deserializer<'de>,
    {
        Report::deserializer()
            .with_modality::<Text>()
            .with_modality::<Tabular>()
            .with_modality::<Image>()
            .with_modality::<Audio>()
            .deserialize(deserializer)
    }

    /// Record each entity's operator *pick* onto its audit trail,
    /// without applying anything.
    ///
    /// Runs at the end of analyze so the returned the audit answers
    /// "what would happen to this entity, and why" before a reviewer
    /// decides anything. Each covered entity gains a [`Selection`]
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
    /// [`Selection`]: elide::entity::audit::AuditKind::Selection
    pub fn record_picks(
        &self,
        context: &DocumentContext,
        policies: &[PolicyDefinition],
        correlation_id: Uuid,
        report: &mut Report,
    ) -> Result<()> {
        // Same gate the orchestrator builders run. This method is a
        // public entry point that *writes* to `report`, so skipping
        // it would stamp `Selection` events resolved against the
        // wrong labelset rather than failing: a duplicate scope name
        // makes a `LabelInScope` rule pick one labelset while the
        // catalog unions both, and the reviewer would act on the
        // misleading provenance.
        validate_scope_references(policies)?;
        let catalog = compile_catalog(policies)?;
        let scope = build_scope(context, catalog.clone(), correlation_id);
        let picker = pickers(&catalog, policies)?;
        record_into(&picker, report, &scope);
        Ok(())
    }
}

/// Reject a request whose rule references a [`LabelScope`] name
/// its own policy didn't declare.
///
/// Scopes are local to the policy that owns them (strict
/// per-policy namespace). A rule inside policy A can reference
/// only scopes declared in policy A's own [`scopes`]: not scopes
/// declared by policy B. Also rejects a policy declaring the same
/// scope name twice. Runs before catalog compilation
/// so an authoring typo (`"gdpr_arcticle_9"`) surfaces as a
/// [`Configuration`](ErrorKind::Configuration) error at request
/// validation time, not as a silent underfire at apply time.
///
/// [`LabelScope`]: elide_governance::LabelScope
/// [`scopes`]: elide_governance::PolicyDefinition::scopes
fn validate_scope_references(policies: &[PolicyDefinition]) -> Result<()> {
    for policy in policies {
        let mut known: HashSet<&str> = HashSet::new();
        for declared in &policy.scopes {
            // Duplicate names would make a `LabelInScope` rule
            // resolve one labelset while `label_scope()` unions
            // both, so recognition and redaction would disagree
            // about what the name means.
            if !known.insert(declared.name.as_str()) {
                return Err(Error::new(
                    ErrorKind::Configuration,
                    format!(
                        "policy `{}` declares scope `{}` more than once; scope names \
                         must be unique within a policy",
                        policy.id,
                        declared.name.as_str(),
                    ),
                ));
            }
        }
        for rule in &policy.rules {
            for (predicate, _) in rule.attachments() {
                check_predicate_scopes(&predicate, &known, policy, rule)?;
            }
        }
    }
    Ok(())
}

/// Walk a predicate tree; every [`Predicate::LabelInScope`] leaf
/// must name a scope declared by the enclosing policy. Returns
/// the first unknown reference with policy + rule context for the
/// error message.
fn check_predicate_scopes(
    predicate: &Predicate,
    known: &HashSet<&str>,
    policy: &PolicyDefinition,
    rule: &PolicyRule,
) -> Result<()> {
    match predicate {
        Predicate::LabelInScope { scope } if !known.contains(scope.as_str()) => Err(Error::new(
            ErrorKind::Configuration,
            format!(
                "policy `{}` rule `{}` references unknown label scope `{}`: \
                 the enclosing policy declares no `LabelScope` with that name",
                policy.id, rule.id, scope,
            ),
        )),
        Predicate::All { all } => all
            .iter()
            .try_for_each(|p| check_predicate_scopes(p, known, policy, rule)),
        Predicate::Any { any } => any
            .iter()
            .try_for_each(|p| check_predicate_scopes(p, known, policy, rule)),
        Predicate::Not { not } => check_predicate_scopes(not, known, policy, rule),
        _ => Ok(()),
    }
}

/// Combine the caller's [`DocumentContext`] with a freshly-derived
/// [`LabelCatalog`] into the elide-facing [`Scope`], traced under
/// `run_id`.
///
/// `run_id` is the id of the document this call is processing, and
/// it tags the tracing span. It is passed rather than read off
/// `context` because the context is a record of what analyze saw:
/// anonymize traces under the document it was handed, without
/// rewriting what analyze wrote.
fn build_scope(context: &DocumentContext, catalog: LabelCatalog, run_id: Uuid) -> Scope {
    Scope {
        languages: context.languages.clone(),
        countries: context.countries.clone(),
        metadata: context.metadata.clone(),
        catalog,
        correlation_id: Some(run_id),
    }
}

/// Record every entity's operator pick onto its own trail, across
/// the body and every container part.
///
/// Each modality's anonymizer sees only its own entities, so a
/// container whose parts span modalities is picked correctly without
/// the caller sorting them first.
fn record_into(pickers: &Pickers, report: &mut Report, scope: &Scope) {
    pick_body(&pickers.text, report, scope);
    pick_body(&pickers.tabular, report, scope);
    pick_body(&pickers.image, report, scope);
    pick_body(&pickers.audio, report, scope);

    let part_ids: Vec<PartId> = report.part_ids().map(|(id, _)| id.clone()).collect();
    for id in part_ids {
        pick_part(&pickers.text, report, &id, scope);
        pick_part(&pickers.tabular, report, &id, scope);
        pick_part(&pickers.image, report, &id, scope);
        pick_part(&pickers.audio, report, &id, scope);
    }
}

/// Run `anonymizer`'s pick pass over the report body, when the body
/// is this anonymizer's modality. A no-op otherwise.
fn pick_body<M: RedactableModality + 'static>(
    anonymizer: &Anonymizer<M>,
    report: &mut Report,
    scope: &Scope,
) {
    if let Some(entities) = report.entities_mut::<M>() {
        anonymizer.pick(entities, scope);
    }
}

/// The part counterpart to [`pick_body`].
fn pick_part<M: RedactableModality + 'static>(
    anonymizer: &Anonymizer<M>,
    report: &mut Report,
    id: &PartId,
    scope: &Scope,
) {
    if let Some(entities) = report.part_entities_mut::<M>(id) {
        anonymizer.pick(entities, scope);
    }
}
