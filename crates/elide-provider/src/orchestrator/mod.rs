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
mod override_set;
mod request;

use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use elide::codec::{FormatRegistry, PartId};
use elide::entity::LabelCatalog;
use elide::modality::Modality;
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
pub use self::override_set::{Override, Overrides};
pub use self::request::RequestContext;
use crate::recognition::{
    Component, Enrichers, Recognizers, compile_audio, compile_image, compile_tabular, compile_text,
};
use crate::redaction::{
    TextOperatorContext, attach_override_audio, attach_override_image, attach_override_tabular,
    attach_override_text, attach_policies_audio, attach_policies_image, attach_policies_tabular,
    attach_policies_text,
};

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
    ) -> Result<Orchestrator<'_>> {
        validate_scope_references(policies)?;
        let catalog = compile_catalog(policies)?;
        let live_scope = build_scope(context, catalog, correlation_id);

        let ner = &self.inner.recognizers.ner;
        let llm = &self.inner.recognizers.llm;
        let ocr = pick_one(&self.inner.enrichers.ocr, "OCR")?;
        let stt = pick_one(&self.inner.enrichers.stt, "STT")?;

        // Analyzers only: analyze recognizes, it does not redact,
        // so `with_analyzer` defaults each anonymizer half rather
        // than us building four that go unused.
        let orchestrator = Orchestrator::new(&self.inner.formats)
            .with_scope(live_scope)
            .with_analyzer::<Text>(compile_text(ner, llm)?)
            .with_analyzer::<Tabular>(compile_tabular(ner)?)
            .with_analyzer::<Image>(compile_image(ner, llm, ocr)?)
            .with_analyzer::<Audio>(compile_audio(ner, stt)?);

        Ok(orchestrator)
    }

    /// Build an [`Orchestrator`] for the anonymize path: reuse
    /// the persisted [`DocumentContext`] from analyze, re-derive the
    /// label catalog from `policies`, wire the requested
    /// `policies` + reviewer `overrides` onto every modality's
    /// anonymizer, and skip the analyzer compile (analysis
    /// already happened; only [`Anonymizer`] state matters here).
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
        overrides: &Overrides,
        key: Option<&KeyConfig>,
        correlation_id: Uuid,
    ) -> Result<Orchestrator<'_>> {
        validate_scope_references(policies)?;
        validate_override_authorities(policies, overrides)?;
        let catalog = compile_catalog(policies)?;
        let live_scope = build_scope(context, catalog.clone(), correlation_id);

        // Fresh per-request text-operator context. Pseudonym
        // vaults materialise per-policy on first access so two
        // policies pseudonymising the same entity don't share a
        // surrogate namespace. `HmacHash`/`Encrypt` resolve their
        // `KeyProvider` from the key this request supplied; a
        // request that supplies none fails at compile time if a
        // policy names either operator. Cross-request pseudonym
        // consistency is a durable-vault story (see elide #143).
        let text_ctx = TextOperatorContext::new(key.map(KeyConfig::build));

        // Overrides attach ahead of the policy rules on every
        // modality: elide's anonymizer is first-match, so that
        // ordering *is* reviewer precedence.
        let text = attach_overrides(
            empty_anonymizer::<Text>(&catalog),
            &overrides.text,
            |a, id, policy, action| attach_override_text(a, id, policy, action, &text_ctx),
        )?;
        let text = attach_policies_text(text, policies.iter(), &text_ctx)?;

        let tabular = attach_overrides(
            empty_anonymizer::<Tabular>(&catalog),
            &overrides.tabular,
            |a, id, policy, action| attach_override_tabular(a, id, policy, action, &text_ctx),
        )?;
        let tabular = attach_policies_tabular(tabular, policies.iter(), &text_ctx)?;

        let image = attach_overrides(
            empty_anonymizer::<Image>(&catalog),
            &overrides.image,
            attach_override_image,
        )?;
        let image = attach_policies_image(image, policies.iter())?;

        let audio = attach_overrides(
            empty_anonymizer::<Audio>(&catalog),
            &overrides.audio,
            attach_override_audio,
        )?;
        let audio = attach_policies_audio(audio, policies.iter())?;

        // Anonymizers only: analysis already happened, so
        // `with_anonymizer` defaults each analyzer half rather
        // than us constructing four empties to satisfy a type.
        let orchestrator = Orchestrator::new(&self.inner.formats)
            .with_scope(live_scope)
            .with_anonymizer::<Text>(text)
            .with_anonymizer::<Tabular>(tabular)
            .with_anonymizer::<Image>(image)
            .with_anonymizer::<Audio>(audio);

        Ok(orchestrator)
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
    /// detected: the pick would first appear after apply, when it is
    /// too late to override. Overrides are deliberately not passed:
    /// none exist yet at analyze time, so this records what the
    /// policy set alone would do.
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
        // No key: a pick only names the operator that *would* run,
        // and no key material is needed to name one. But elide
        // compiles the operator to reach its name, so a policy using
        // `HmacHash`/`Encrypt` still fails here rather than
        // recording a keyless pick. That is why the analyze path
        // tolerates this method failing: the key legitimately does
        // not arrive until anonymize.
        let text_ctx = TextOperatorContext::new(None);

        // No overrides: none exist yet at analyze time, so this
        // records what the policy set alone would do.
        let picker = Picker {
            text: attach_policies_text(empty_anonymizer(&catalog), policies.iter(), &text_ctx)?,
            tabular: attach_policies_tabular(
                empty_anonymizer(&catalog),
                policies.iter(),
                &text_ctx,
            )?,
            image: attach_policies_image(empty_anonymizer(&catalog), policies.iter())?,
            audio: attach_policies_audio(empty_anonymizer(&catalog), policies.iter())?,
        };

        picker.record_into(report, &scope);
        Ok(())
    }
}

/// Reject a request whose reviewer override names a policy id
/// no submitted policy carries.
///
/// Overrides inherit the authority of the policy they name: the
/// audit event stamps that policy, and any per-policy operator
/// infrastructure (pseudonym vault, `KeyProvider`) is looked up
/// under that policy id. An override that names a non-existent
/// policy would attribute to nothing and: worse: silently draw
/// from an empty per-policy vault or fall back to the engine
/// default `KeyProvider`, both of which are the wrong authority.
fn validate_override_authorities(
    policies: &[PolicyDefinition],
    overrides: &Overrides,
) -> Result<()> {
    let known: HashSet<Uuid> = policies.iter().map(|p| p.id).collect();
    for (entity_id, policy_id) in overrides.authorities() {
        if !known.contains(&policy_id) {
            return Err(Error::new(
                ErrorKind::Configuration,
                format!(
                    "override for entity `{entity_id}` names policy `{policy_id}` that \
                     no submitted policy in this request carries; overrides inherit a \
                     policy's authority and must name one that's actually loaded",
                ),
            ));
        }
    }
    Ok(())
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

/// The single enricher a lineup may wire, or `None` for an empty
/// one.
///
/// elide attaches at most one enricher per analyzer, so a lineup
/// naming two is a misconfiguration worth rejecting at request
/// compile rather than silently running the first. `kind` names the
/// lineup in that error.
fn pick_one<'a, B>(lineup: &'a [Component<B>], kind: &str) -> Result<Option<&'a Component<B>>> {
    match lineup {
        [] => Ok(None),
        [one] => Ok(Some(one)),
        many => Err(Error::new(
            ErrorKind::Configuration,
            format!(
                "{kind} enricher lineup carries {} entries; elide attaches at most \
                 one per analyzer. Wire exactly one enricher.",
                many.len(),
            ),
        )),
    }
}

/// An anonymizer knowing the request's label vocabulary and nothing
/// else: no policies, no overrides.
///
/// The starting point every redacting anonymizer is built from.
fn empty_anonymizer<M>(catalog: &LabelCatalog) -> Anonymizer<M>
where
    M: Modality + 'static,
{
    Anonymizer::<M>::new().with_catalog(catalog.clone())
}

/// Layer this modality's reviewer overrides onto `anonymizer`.
///
/// Call before attaching the policy rules. elide's anonymizer is
/// first-match, so overrides attached ahead of the rules *are* the
/// precedence: where a reviewer named an operator for one entity,
/// that choice wins over whatever the policy set would have picked.
/// Attach them after, and reviewers are silently ignored.
///
/// Iteration order does not matter: each override attaches a rule
/// matching one entity id, so no two can ever claim the same entity.
fn attach_overrides<M, F>(
    mut anonymizer: Anonymizer<M>,
    overrides: &HashMap<Uuid, Override<M>>,
    attach_one: F,
) -> Result<Anonymizer<M>>
where
    M: RedactableModality + 'static,
    F: Fn(Anonymizer<M>, Uuid, Uuid, &M::Redaction) -> Result<Anonymizer<M>>,
{
    for (entity_id, over) in overrides {
        anonymizer = attach_one(anonymizer, *entity_id, over.policy_id, &over.action)?;
    }
    Ok(anonymizer)
}

/// The four per-modality anonymizers a pick pass runs through,
/// assembled once per request.
pub(crate) struct Picker {
    pub(crate) text: Anonymizer<Text>,
    pub(crate) tabular: Anonymizer<Tabular>,
    pub(crate) image: Anonymizer<Image>,
    pub(crate) audio: Anonymizer<Audio>,
}

impl Picker {
    /// Record every entity's operator pick onto its own trail,
    /// across the body and every container part.
    ///
    /// Each modality's anonymizer sees only its own entities, so a
    /// container whose parts span modalities is picked correctly
    /// without the caller sorting them first.
    fn record_into(&self, report: &mut Report, scope: &Scope) {
        pick_body(&self.text, report, scope);
        pick_body(&self.tabular, report, scope);
        pick_body(&self.image, report, scope);
        pick_body(&self.audio, report, scope);

        let part_ids: Vec<PartId> = report.part_ids().map(|(id, _)| id.clone()).collect();
        for id in part_ids {
            pick_part(&self.text, report, &id, scope);
            pick_part(&self.tabular, report, &id, scope);
            pick_part(&self.image, report, &id, scope);
            pick_part(&self.audio, report, &id, scope);
        }
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
