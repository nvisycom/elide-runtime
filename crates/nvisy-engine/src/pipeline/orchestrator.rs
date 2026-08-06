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

use std::slice;

use elide::detection::Analyzer;
use elide::recognition::Scope;
use elide::redaction::Anonymizer;
use elide::{Error, Orchestrator, Result};
use elide_core::entity::LabelCatalog;
use elide_core::modality::Modality;
#[cfg(feature = "internal_audio")]
use elide_core::modality::audio::Audio;
#[cfg(feature = "internal_image")]
use elide_core::modality::image::Image;
#[cfg(feature = "internal_tabular")]
use elide_core::modality::tabular::Tabular;
use elide_core::modality::text::Text;
use nvisy_schema::plan::AnalyzerParams;
use nvisy_schema::policy::PolicyDefinition;
use nvisy_schema::policy::redaction::ModalityRedactions;
use uuid::Uuid;

use super::Engine;
use super::audit::AuditContext;
use crate::analyzer::{AnalyzerCompile, compile_catalog};
#[cfg(feature = "internal_audio")]
use crate::anonymizer::{attach_override_audio, attach_policies_audio};
#[cfg(feature = "internal_image")]
use crate::anonymizer::{attach_override_image, attach_policies_image};
#[cfg(feature = "internal_tabular")]
use crate::anonymizer::{attach_override_tabular, attach_policies_tabular};
use crate::anonymizer::{attach_override_text, attach_policies_text};

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
    /// [`PolicyDefinition::labels`]: nvisy_schema::policy::PolicyDefinition::labels
    pub(super) fn build_analyze_orchestrator(
        &self,
        spec: &AnalyzerParams,
        policies: &[PolicyDefinition],
        correlation_id: Uuid,
    ) -> Result<(Orchestrator<'_>, AuditContext)> {
        let catalog = compile_catalog(policies);
        let context = AuditContext {
            languages: spec.scope.languages.clone(),
            countries: spec.scope.countries.clone(),
            tags: spec.scope.tags.clone(),
            correlation_id,
        };
        let live_scope = build_scope(&context, catalog.clone(), correlation_id);

        let text_anon = assemble_empty::<Text>(&catalog);
        let text_analyzer = spec.compile_text(&self.ner, &self.llm, &self.pattern_guardrails)?;

        let orchestrator = Orchestrator::new(&self.formats)
            .with_scope(live_scope)
            .with_modality::<Text>(text_analyzer, text_anon);

        #[cfg(feature = "internal_tabular")]
        let orchestrator = {
            let anon = assemble_empty::<Tabular>(&catalog);
            let analyzer = spec.compile_tabular(&self.ner, &self.pattern_guardrails)?;
            orchestrator.with_modality::<Tabular>(analyzer, anon)
        };

        #[cfg(feature = "internal_image")]
        let orchestrator = {
            let anon = assemble_empty::<Image>(&catalog);
            let analyzer = spec.compile_image(&self.ner, &self.llm, &self.pattern_guardrails)?;
            orchestrator.with_modality::<Image>(analyzer, anon)
        };

        #[cfg(feature = "internal_audio")]
        let orchestrator = {
            let anon = assemble_empty::<Audio>(&catalog);
            let analyzer = spec.compile_audio(&self.ner, &self.pattern_guardrails)?;
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
        overrides: &[(Uuid, ModalityRedactions)],
        correlation_id: Uuid,
    ) -> Result<Orchestrator<'_>> {
        let catalog = compile_catalog(policies);
        let live_scope = build_scope(context, catalog.clone(), correlation_id);

        let text_anon = assemble::<Text, _, _>(
            &catalog,
            overrides,
            policies,
            attach_override_text,
            attach_policies_text,
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
                attach_override_tabular,
                attach_policies_tabular,
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

/// Combine engine-facing [`AuditContext`] with a freshly-derived
/// [`LabelCatalog`] and the current phase's `correlation_id` into
/// the elide-facing [`Scope`].
fn build_scope(context: &AuditContext, catalog: LabelCatalog, correlation_id: Uuid) -> Scope {
    Scope {
        languages: context.languages.clone(),
        countries: context.countries.clone(),
        tags: context.tags.clone(),
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
fn assemble<'a, M, O, P>(
    catalog: &LabelCatalog,
    overrides: &[(Uuid, ModalityRedactions)],
    policies: &'a [PolicyDefinition],
    attach_override: O,
    attach_policies: P,
) -> Result<Anonymizer<M>>
where
    M: Modality + 'static,
    O: Fn(Anonymizer<M>, Uuid, &ModalityRedactions) -> Result<Anonymizer<M>, Error>,
    P: FnOnce(Anonymizer<M>, slice::Iter<'a, PolicyDefinition>) -> Result<Anonymizer<M>, Error>,
{
    let mut anonymizer = Anonymizer::<M>::new().with_catalog(catalog.clone());
    for (id, action) in overrides {
        anonymizer = attach_override(anonymizer, *id, action)?;
    }
    attach_policies(anonymizer, policies.iter())
}
