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

use std::collections::{HashMap, HashSet};
use std::slice;

use elide::codec::PartId;
use elide::detection::Analyzer;
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
use elide_governance::{PolicyDefinition, PolicyRule, Predicate};
use uuid::Uuid;

use crate::catalog::compile_catalog;
use crate::context::AuditContext;
use crate::plan::AnalyzerParams;
use crate::recognition::{
    OcrConfig, OcrEnricherConfig, SttConfig, SttEnricherConfig, compile_audio, compile_image,
    compile_tabular, compile_text,
};
use crate::redaction::{
    TextOperatorContext, attach_override_audio, attach_override_image, attach_override_tabular,
    attach_override_text, attach_policies_audio, attach_policies_image, attach_policies_tabular,
    attach_policies_text,
};
use crate::{Override, Overrides, Provider};

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
    /// Returns both the orchestrator and the resolved
    /// [`AuditContext`] so the caller can persist the
    /// caller-asserted scope + analyze-side correlation id onto
    /// the returned the audit. The orchestrator's own
    /// scope carries the same `correlation_id` for tracing spans.
    ///
    /// [`Scope`]: elide::recognition::Scope
    /// [`LabelCatalog`]: elide::entity::LabelCatalog
    /// [`PolicyDefinition::label_scope`]: elide_governance::PolicyDefinition::label_scope
    pub fn analyze_orchestrator(
        &self,
        spec: &AnalyzerParams,
        policies: &[PolicyDefinition],
        correlation_id: Uuid,
    ) -> Result<(Orchestrator<'_>, AuditContext)> {
        validate_scope_references(policies)?;
        let catalog = compile_catalog(policies)?;
        let context = AuditContext {
            languages: spec.scope.languages.clone(),
            countries: spec.scope.countries.clone(),
            metadata: spec.scope.metadata.clone(),
            correlation_id,
            raster_mode: spec.raster_mode,
        };
        let live_scope = build_scope(&context, catalog.clone(), correlation_id);

        let text_anon = assemble_empty::<Text>(&catalog);
        let text_analyzer = compile_text(&self.ner, &self.llm)?;

        let orchestrator = Orchestrator::new(&self.formats)
            .with_scope(live_scope)
            .with_modality::<Text>(text_analyzer, text_anon);

        let orchestrator = {
            let anon = assemble_empty::<Tabular>(&catalog);
            let analyzer = compile_tabular(&self.ner)?;
            orchestrator.with_modality::<Tabular>(analyzer, anon)
        };

        let orchestrator = {
            let anon = assemble_empty::<Image>(&catalog);
            let analyzer = compile_image(&self.ner, &self.llm, pick_ocr(&self.ocr)?)?;
            orchestrator.with_modality::<Image>(analyzer, anon)
        };

        let orchestrator = {
            let anon = assemble_empty::<Audio>(&catalog);
            let analyzer = compile_audio(&self.ner, pick_stt(&self.stt)?)?;
            orchestrator.with_modality::<Audio>(analyzer, anon)
        };

        Ok((orchestrator, context))
    }

    /// Build an [`Orchestrator`] for the anonymize path: reuse
    /// the persisted [`AuditContext`] from analyze, re-derive the
    /// label catalog from `policies`, wire the requested
    /// `policies` + reviewer `overrides` onto every modality's
    /// anonymizer, and skip the analyzer compile (analysis
    /// already happened; only [`Anonymizer`] state matters here).
    ///
    /// Each modality gets an empty [`Analyzer<M>`]; elide's
    /// [`Orchestrator::anonymize_with`] doesn't run recognition on
    /// this path, so an empty analyzer is a zero-cost placeholder
    /// needed only to satisfy `with_modality`'s type contract.
    ///
    /// The `correlation_id` from the anonymize-time
    /// document is stamped fresh onto the returned
    /// orchestrator's scope so anonymize-side tracing spans are
    /// distinct from the analyze-side ones on `context`.
    ///
    /// [`Analyzer<M>`]: elide::detection::Analyzer
    pub fn anonymize_orchestrator(
        &self,
        context: &AuditContext,
        policies: &[PolicyDefinition],
        overrides: &Overrides,
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
        // `KeyProvider` through the engine-level default. Cross-
        // request pseudonym consistency is a durable-vault story
        // (see elide #143).
        let text_ctx = TextOperatorContext::new(self.key_provider.clone());

        let text_anon = assemble::<Text, _, _>(
            &catalog,
            &overrides.text,
            policies,
            |anon, id, policy, action| attach_override_text(anon, id, policy, action, &text_ctx),
            |anon, policies| attach_policies_text(anon, policies, &text_ctx),
        )?;

        let orchestrator = Orchestrator::new(&self.formats)
            .with_scope(live_scope)
            .with_modality::<Text>(Analyzer::<Text>::new(), text_anon);

        let orchestrator = {
            let anon = assemble::<Tabular, _, _>(
                &catalog,
                &overrides.tabular,
                policies,
                |anon, id, policy, action| {
                    attach_override_tabular(anon, id, policy, action, &text_ctx)
                },
                |anon, policies| attach_policies_tabular(anon, policies, &text_ctx),
            )?;
            orchestrator.with_modality::<Tabular>(Analyzer::<Tabular>::new(), anon)
        };

        let orchestrator = {
            let anon = assemble::<Image, _, _>(
                &catalog,
                &overrides.image,
                policies,
                attach_override_image,
                attach_policies_image,
            )?;
            orchestrator.with_modality::<Image>(Analyzer::<Image>::new(), anon)
        };

        let orchestrator = {
            let anon = assemble::<Audio, _, _>(
                &catalog,
                &overrides.audio,
                policies,
                attach_override_audio,
                attach_policies_audio,
            )?;
            orchestrator.with_modality::<Audio>(Analyzer::<Audio>::new(), anon)
        };

        Ok(orchestrator)
    }

    /// An orchestrator carrying only the modality registry, for
    /// rebuilding a serialized [`Report`].
    ///
    /// Deserialization routes each entity group to its modality by
    /// name and needs nothing else: no policies, no operators, no
    /// scope. The analyzers and anonymizers are the empty ones
    /// [`with_modality`] insists on.
    ///
    /// [`with_modality`]: elide::Orchestrator::with_modality
    pub fn report_orchestrator(&self) -> Orchestrator<'_> {
        let catalog = LabelCatalog::new();
        Orchestrator::new(&self.formats)
            .with_modality::<Text>(Analyzer::<Text>::new(), assemble_empty(&catalog))
            .with_modality::<Tabular>(Analyzer::<Tabular>::new(), assemble_empty(&catalog))
            .with_modality::<Image>(Analyzer::<Image>::new(), assemble_empty(&catalog))
            .with_modality::<Audio>(Analyzer::<Audio>::new(), assemble_empty(&catalog))
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
    /// **Never fails the analyze it runs inside.** A pick is
    /// informational, so a policy whose operators cannot be
    /// compiled here (an `HmacHash` with no [`KeyProvider`] wired,
    /// say) simply yields no pick, and the same policy still fails
    /// loudly at anonymize where the operator would actually run.
    /// Refusing to analyze a document because a redaction the
    /// caller has not asked for yet is misconfigured would deny
    /// them the detections too.
    ///
    /// [`KeyProvider`]: elide::redaction::operators::KeyProvider
    /// [`Selection`]: elide::entity::audit::AuditKind::Selection
    pub fn record_picks(
        &self,
        context: &AuditContext,
        policies: &[PolicyDefinition],
        correlation_id: Uuid,
        report: &mut Report,
    ) {
        // Deliberately discarded, not swallowed: every reason this
        // can fail (an unresolvable label, an operator with no
        // capability wired) is raised again by `anonymize`, which
        // compiles the same policies and does fail. Reporting it here
        // would surface each one twice and turn analyze into a second
        // place a caller must handle redaction errors. The observable
        // signal is that the audit carries no `Selection` events.
        let _ = self.try_record_picks(context, policies, correlation_id, report);
    }

    /// The fallible body of [`record_picks`](Self::record_picks).
    fn try_record_picks(
        &self,
        context: &AuditContext,
        policies: &[PolicyDefinition],
        correlation_id: Uuid,
        report: &mut Report,
    ) -> Result<()> {
        let catalog = compile_catalog(policies)?;
        let scope = build_scope(context, catalog.clone(), correlation_id);
        let text_ctx = TextOperatorContext::new(self.key_provider.clone());
        let picker = Picker {
            text: assemble::<Text, _, _>(
                &catalog,
                &HashMap::new(),
                policies,
                no_override,
                |anon, p| attach_policies_text(anon, p, &text_ctx),
            )?,
            tabular: assemble::<Tabular, _, _>(
                &catalog,
                &HashMap::new(),
                policies,
                no_override,
                |anon, p| attach_policies_tabular(anon, p, &text_ctx),
            )?,
            image: assemble::<Image, _, _>(
                &catalog,
                &HashMap::new(),
                policies,
                no_override,
                attach_policies_image,
            )?,
            audio: assemble::<Audio, _, _>(
                &catalog,
                &HashMap::new(),
                policies,
                no_override,
                attach_policies_audio,
            )?,
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

/// Combine engine-facing [`AuditContext`] with a freshly-derived
/// [`LabelCatalog`] and the current phase's `correlation_id` into
/// the elide-facing [`Scope`].
fn build_scope(context: &AuditContext, catalog: LabelCatalog, correlation_id: Uuid) -> Scope {
    Scope {
        languages: context.languages.clone(),
        countries: context.countries.clone(),
        metadata: context.metadata.clone(),
        catalog,
        correlation_id: Some(correlation_id),
    }
}

/// Empty per-modality anonymizer: catalog only, no policies,
/// no overrides. Used on the analyze path where redaction isn't
/// yet in play; [`Anonymizer`] presence is required by
/// [`Orchestrator::with_modality`]'s type contract.
fn assemble_empty<M>(catalog: &LabelCatalog) -> Anonymizer<M>
where
    M: Modality + 'static,
{
    Anonymizer::<M>::new().with_catalog(catalog.clone())
}

/// Assemble one modality's anonymizer: seed with the catalog,
/// layer reviewer overrides (so overrides win over policy rules),
/// then attach the policy chain.
///
/// `attach_override` and `attach_policies` are the per-modality
/// bridges into [`crate::redaction`]; they know which
/// `ModalityRedactions` field to read and how to build the typed
/// operator. This helper owns the invariant order and the error
/// wrapping so each modality's callsite is one call.
/// Pick the single OCR enricher from the engine's lineup, or
/// return `None` when nothing was wired. Rejects a lineup with
/// more than one entry: elide's `Enricher<Image>` attaches at
/// most one OCR enricher per analyzer.
fn pick_ocr(ocr: &OcrConfig) -> Result<Option<&OcrEnricherConfig>> {
    match ocr.enrichers.as_slice() {
        [] => Ok(None),
        [one] => Ok(Some(one)),
        many => Err(Error::new(
            ErrorKind::Configuration,
            format!(
                "OCR enricher lineup carries {} entries; elide attaches at most one \
                 per analyzer today. Wire exactly one enricher.",
                many.len(),
            ),
        )),
    }
}

/// Pick the single STT enricher from the engine's lineup, or
/// return `None` when nothing was wired. Rejects a lineup with
/// more than one entry: elide's `Enricher<Audio>` attaches at
/// most one STT enricher per analyzer.
fn pick_stt(stt: &SttConfig) -> Result<Option<&SttEnricherConfig>> {
    match stt.enrichers.as_slice() {
        [] => Ok(None),
        [one] => Ok(Some(one)),
        many => Err(Error::new(
            ErrorKind::Configuration,
            format!(
                "STT enricher lineup carries {} entries; elide attaches at most one \
                 per analyzer today. Wire exactly one enricher.",
                many.len(),
            ),
        )),
    }
}

/// Build one modality's anonymizer: reviewer overrides first, so
/// they win over the policy rules layered after them.
///
/// Iteration order is unspecified and does not matter: each
/// override attaches a rule matching one entity id, so no two can
/// ever claim the same entity.
fn assemble<'a, M, O, P>(
    catalog: &LabelCatalog,
    overrides: &HashMap<Uuid, Override<M>>,
    policies: &'a [PolicyDefinition],
    attach_override: O,
    attach_policies: P,
) -> Result<Anonymizer<M>>
where
    M: RedactableModality + 'static,
    O: Fn(Anonymizer<M>, Uuid, Uuid, &M::Redaction) -> Result<Anonymizer<M>>,
    P: FnOnce(Anonymizer<M>, slice::Iter<'a, PolicyDefinition>) -> Result<Anonymizer<M>>,
{
    let mut anonymizer = Anonymizer::<M>::new().with_catalog(catalog.clone());
    for (entity_id, over) in overrides {
        anonymizer = attach_override(anonymizer, *entity_id, over.policy_id, &over.action)?;
    }
    attach_policies(anonymizer, policies.iter())
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

/// The `attach_override` argument for a pick pass: overrides do
/// not exist yet at analyze time, so this is never called. Named
/// rather than a closure so all four `assemble` calls share it.
fn no_override<M: RedactableModality + 'static>(
    anonymizer: Anonymizer<M>,
    _entity_id: Uuid,
    _policy_id: Uuid,
    _action: &M::Redaction,
) -> Result<Anonymizer<M>> {
    Ok(anonymizer)
}
