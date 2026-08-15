//! Build a per-request [`Orchestrator`] from either an
//! [`AnalyzerParams`] (at analyze time) or a persisted
//! [`AuditContext`] (at anonymize time).
//!
//! One pipeline per modality, all wired against a single
//! [`Scope`]. The orchestrator is built fresh per call: a small
//! map of trait objects keyed by modality `TypeId`. The anonymize
//! path is cheap — empty analyzers, per-request policy + override
//! attachment. The analyze path is not free — LLM recognizers
//! construct their [`RigBackend`] client on every compile — but
//! the per-call shape is what lets us re-resolve policies and
//! overrides per document at anonymize time without mutating a
//! shared anonymizer.
//!
//! [`RigBackend`]: elide::recognition::llm::backend::RigBackend
//!
//! Two entry points:
//!
//! - [`Engine::build_analyze_orchestrator`] compiles the analyzer
//!   chain from an [`AnalyzerParams`]; empty policies + overrides.
//!   Consumed by [`Engine::analyze`].
//! - [`Engine::build_anonymize_orchestrator`] takes the persisted
//!   analyze-time [`AuditContext`]; uses empty analyzers
//!   (recognition already happened, apply just needs the
//!   anonymizer stack) but a full policy + override set. Consumed
//!   by [`Engine::anonymize`].
//!
//! Both entry points build the internal [`Scope`] the same way:
//! merge `AuditContext` facts (languages, countries, tags) with a
//! freshly-derived label catalog and the current phase's
//! correlation id. The catalog is always re-derived from
//! `policies`; there is no persisted catalog anywhere in the
//! engine's public API.
//!
//! [`Scope`]: elide::recognition::Scope

use std::collections::HashSet;
use std::slice;

use elide::detection::Analyzer;
use elide::recognition::Scope;
use elide::redaction::Anonymizer;
use elide::{Orchestrator, Result};
use elide_core::entity::LabelCatalog;
use elide_core::modality::Modality;
#[cfg(feature = "internal_audio")]
use elide_core::modality::audio::Audio;
#[cfg(feature = "internal_image")]
use elide_core::modality::image::Image;
#[cfg(feature = "internal_tabular")]
use elide_core::modality::tabular::Tabular;
use elide_core::modality::text::Text;
use elide_core::{Error, ErrorKind};
use elide_governance::Predicate;
use elide_governance::{PolicyDefinition, PolicyRule};
use elide_wire::plan::AnalyzerParams;
use uuid::Uuid;

use super::Engine;
use super::audit::AuditContext;
#[cfg(feature = "internal_audio")]
use crate::analyzer::compile_audio;
#[cfg(feature = "internal_image")]
use crate::analyzer::compile_image;
#[cfg(feature = "internal_tabular")]
use crate::analyzer::compile_tabular;
use crate::analyzer::{compile_catalog, compile_text};
use crate::anonymizer::{TextOperatorContext, attach_override_text, attach_policies_text};
#[cfg(feature = "internal_audio")]
use crate::anonymizer::{attach_override_audio, attach_policies_audio};
#[cfg(feature = "internal_image")]
use crate::anonymizer::{attach_override_image, attach_policies_image};
#[cfg(feature = "internal_tabular")]
use crate::anonymizer::{attach_override_tabular, attach_policies_tabular};
use crate::entity::OverrideEntry;
#[cfg(feature = "internal_image")]
use crate::provider::ocr::{OcrBackend, OcrConfig};
#[cfg(feature = "internal_audio")]
use crate::provider::stt::{SttBackend, SttConfig};

impl Engine {
    /// Build an [`Orchestrator`] for the analyze path: compile
    /// every per-modality analyzer from `spec`, wire empty
    /// anonymizers (analyze doesn't run redaction), and stamp the
    /// request-scoped [`Scope`].
    ///
    /// The label catalog is derived from `policies`: every
    /// submitted [`PolicyDefinition::labels`] unions into one
    /// [`LabelCatalog`] used to drive recognizer dispatch and
    /// tag-based selector matching.
    ///
    /// Returns both the orchestrator and the resolved
    /// [`AuditContext`] so [`Engine::analyze`] can persist the
    /// caller-asserted scope + analyze-side correlation id onto
    /// the returned [`super::Audit`]. The orchestrator's own
    /// scope carries the same `correlation_id` for tracing spans.
    ///
    /// [`Scope`]: elide::recognition::Scope
    /// [`LabelCatalog`]: elide_core::entity::LabelCatalog
    /// [`PolicyDefinition::labels`]: elide_governance::PolicyDefinition::labels
    pub(super) fn build_analyze_orchestrator(
        &self,
        spec: &AnalyzerParams,
        policies: &[PolicyDefinition],
        correlation_id: Uuid,
    ) -> Result<(Orchestrator<'_>, AuditContext)> {
        validate_group_references(policies)?;
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

        #[cfg(feature = "internal_tabular")]
        let orchestrator = {
            let anon = assemble_empty::<Tabular>(&catalog);
            let analyzer = compile_tabular(&self.ner)?;
            orchestrator.with_modality::<Tabular>(analyzer, anon)
        };

        #[cfg(feature = "internal_image")]
        let orchestrator = {
            let anon = assemble_empty::<Image>(&catalog);
            let analyzer = compile_image(&self.ner, &self.llm, pick_ocr(&self.ocr)?)?;
            orchestrator.with_modality::<Image>(analyzer, anon)
        };

        #[cfg(feature = "internal_audio")]
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
    /// [`super::Document`] is stamped fresh onto the returned
    /// orchestrator's scope so anonymize-side tracing spans are
    /// distinct from the analyze-side ones on `context`.
    ///
    /// [`Analyzer<M>`]: elide::detection::Analyzer
    pub(super) fn build_anonymize_orchestrator(
        &self,
        context: &AuditContext,
        policies: &[PolicyDefinition],
        overrides: &[OverrideEntry],
        correlation_id: Uuid,
    ) -> Result<Orchestrator<'_>> {
        validate_group_references(policies)?;
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
            overrides,
            policies,
            |anon, entry| attach_override_text(anon, entry, &text_ctx),
            |anon, policies| attach_policies_text(anon, policies, &text_ctx),
        )?;

        let orchestrator = Orchestrator::new(&self.formats)
            .with_scope(live_scope)
            .with_modality::<Text>(Analyzer::<Text>::new(), text_anon);

        #[cfg(feature = "internal_tabular")]
        let orchestrator = {
            let anon = assemble::<Tabular, _, _>(
                &catalog,
                overrides,
                policies,
                |anon, entry| attach_override_tabular(anon, entry, &text_ctx),
                |anon, policies| attach_policies_tabular(anon, policies, &text_ctx),
            )?;
            orchestrator.with_modality::<Tabular>(Analyzer::<Tabular>::new(), anon)
        };

        #[cfg(feature = "internal_image")]
        let orchestrator = {
            let anon = assemble::<Image, _, _>(
                &catalog,
                overrides,
                policies,
                attach_override_image,
                attach_policies_image,
            )?;
            orchestrator.with_modality::<Image>(Analyzer::<Image>::new(), anon)
        };

        #[cfg(feature = "internal_audio")]
        let orchestrator = {
            let anon = assemble::<Audio, _, _>(
                &catalog,
                overrides,
                policies,
                attach_override_audio,
                attach_policies_audio,
            )?;
            orchestrator.with_modality::<Audio>(Analyzer::<Audio>::new(), anon)
        };

        Ok(orchestrator)
    }
}

/// Reject a request whose reviewer override names a policy id
/// no submitted policy carries.
///
/// Overrides inherit the authority of the policy they name — the
/// audit event stamps that policy, and any per-policy operator
/// infrastructure (pseudonym vault, `KeyProvider`) is looked up
/// under that policy id. An override that names a non-existent
/// policy would attribute to nothing and — worse — silently draw
/// from an empty per-policy vault or fall back to the engine
/// default `KeyProvider`, both of which are the wrong authority.
fn validate_override_authorities(
    policies: &[PolicyDefinition],
    overrides: &[OverrideEntry],
) -> Result<()> {
    let known: HashSet<Uuid> = policies.iter().map(|p| p.id).collect();
    for entry in overrides {
        if !known.contains(&entry.policy_id) {
            return Err(Error::new(
                ErrorKind::Configuration,
                format!(
                    "override for entity `{}` names policy `{}` that no submitted \
                     policy in this request carries; overrides inherit a policy's \
                     authority and must name one that's actually loaded",
                    entry.entity_id, entry.policy_id,
                ),
            ));
        }
    }
    Ok(())
}

/// Reject a request whose rule references a [`LabelGroup`] name
/// its own policy didn't declare.
///
/// Groups are scoped to the policy that owns them (strict
/// per-policy namespace). A rule inside policy A can reference
/// only groups declared in policy A's own [`groups`] slot — not
/// groups declared by policy B. Runs before catalog compilation
/// so an authoring typo (`"gdpr_arcticle_9"`) surfaces as a
/// [`Configuration`](ErrorKind::Configuration) error at request
/// validation time, not as a silent underfire at apply time.
///
/// [`LabelGroup`]: elide_governance::LabelGroup
/// [`groups`]: elide_governance::PolicyDefinition::groups
fn validate_group_references(policies: &[PolicyDefinition]) -> Result<()> {
    for policy in policies {
        let known: HashSet<&str> = policy.groups.iter().map(|g| g.name.as_str()).collect();
        for rule in &policy.rules {
            for (predicate, _) in rule.attachments() {
                check_predicate_groups(&predicate, &known, policy, rule)?;
            }
        }
    }
    Ok(())
}

/// Walk a predicate tree; every [`Predicate::LabelInGroup`] leaf
/// must name a group declared by the enclosing policy. Returns
/// the first unknown reference with policy + rule context for the
/// error message.
fn check_predicate_groups(
    predicate: &Predicate,
    known: &HashSet<&str>,
    policy: &PolicyDefinition,
    rule: &PolicyRule,
) -> Result<()> {
    match predicate {
        Predicate::LabelInGroup { group } if !known.contains(group.as_str()) => Err(Error::new(
            ErrorKind::Configuration,
            format!(
                "policy `{}` rule `{}` references unknown label group `{}` — \
                 the enclosing policy declares no `LabelGroup` with that name",
                policy.id, rule.id, group,
            ),
        )),
        Predicate::All { all } => all
            .iter()
            .try_for_each(|p| check_predicate_groups(p, known, policy, rule)),
        Predicate::Any { any } => any
            .iter()
            .try_for_each(|p| check_predicate_groups(p, known, policy, rule)),
        Predicate::Not { not } => check_predicate_groups(not, known, policy, rule),
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
/// bridges into [`crate::anonymizer`]; they know which
/// `ModalityRedactions` field to read and how to build the typed
/// operator. This helper owns the invariant order and the error
/// wrapping so each modality's callsite is one call.
/// Pick the single OCR backend from the engine's lineup, or
/// return `None` when nothing was wired. Rejects a lineup with
/// more than one entry — elide's `Enricher<Image>` attaches at
/// most one OCR enricher per analyzer.
#[cfg(feature = "internal_image")]
fn pick_ocr(ocr: &OcrConfig) -> Result<Option<&OcrBackend>> {
    match ocr.enrichers.as_slice() {
        [] => Ok(None),
        [one] => Ok(Some(&one.backend)),
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

/// Pick the single STT backend from the engine's lineup, or
/// return `None` when nothing was wired. Rejects a lineup with
/// more than one entry — elide's `Enricher<Audio>` attaches at
/// most one STT enricher per analyzer.
#[cfg(feature = "internal_audio")]
fn pick_stt(stt: &SttConfig) -> Result<Option<&SttBackend>> {
    match stt.enrichers.as_slice() {
        [] => Ok(None),
        [one] => Ok(Some(&one.backend)),
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

fn assemble<'a, M, O, P>(
    catalog: &LabelCatalog,
    overrides: &[OverrideEntry],
    policies: &'a [PolicyDefinition],
    attach_override: O,
    attach_policies: P,
) -> Result<Anonymizer<M>>
where
    M: Modality + 'static,
    O: Fn(Anonymizer<M>, &OverrideEntry) -> Result<Anonymizer<M>>,
    P: FnOnce(Anonymizer<M>, slice::Iter<'a, PolicyDefinition>) -> Result<Anonymizer<M>>,
{
    let mut anonymizer = Anonymizer::<M>::new().with_catalog(catalog.clone());
    for entry in overrides {
        anonymizer = attach_override(anonymizer, entry)?;
    }
    attach_policies(anonymizer, policies.iter())
}
