//! Build a per-request [`Orchestrator`] from either an
//! [`AnalyzerParams`] (at analyze time) or a persisted
//! [`Scope`] (at anonymize time).
//!
//! One pipeline per modality, all wired against a single
//! [`Scope`]. The orchestrator is built fresh per call: a small
//! map of trait objects keyed by modality `TypeId`. The
//! anonymize path is cheap — empty analyzers, per-request
//! policy + override attachment. The analyze path is not free —
//! LLM recognizers construct their [`RigBackend`] client on
//! every compile — but the per-call shape is what lets us
//! re-resolve policies and overrides per document at anonymize
//! time without mutating a shared anonymizer.
//!
//! [`RigBackend`]: elide::recognition::llm::backend::RigBackend
//!
//! Two entry points:
//!
//! - [`Engine::build_analyze_orchestrator`] compiles the
//!   analyzer chain from an [`AnalyzerParams`]; empty policies +
//!   overrides. Consumed by [`Engine::analyze_document`].
//! - [`Engine::build_anonymize_orchestrator`] takes the
//!   persisted analyze-time [`Scope`] as-is; uses empty
//!   analyzers (recognition already happened, apply just needs
//!   the anonymizer stack) but a full policy + override set.
//!   Consumed by [`Engine::anonymize_document`].
//!
//! [`Scope`]: elide::recognition::Scope

use crate::Result;
use elide::Orchestrator;
use elide::detection::Analyzer;
use elide::recognition::Scope;
use elide::redaction::Anonymizer;
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
use nvisy_schema::policy::{Policy, PolicyAction};
use uuid::Uuid;

use super::Engine;
use crate::analyzer::{AnalyzerCompile, LabelCatalogCompile};
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
    /// anonymizers (analyze doesn't run redaction), and stamp
    /// the request-scoped [`Scope`].
    ///
    /// Returns both the orchestrator and the resolved [`Scope`]
    /// so [`Engine::analyze_document`] can persist the scope onto
    /// the returned [`super::AnalyzedDocument`] with
    /// `correlation_id: None`. The orchestrator itself carries
    /// the caller-supplied `correlation_id` for tracing spans.
    ///
    /// [`Scope`]: elide::recognition::Scope
    pub(super) fn build_analyze_orchestrator(
        &self,
        spec: &AnalyzerParams,
        correlation_id: Uuid,
    ) -> Result<(Orchestrator<'_>, Scope)> {
        let catalog = spec.scope.label_catalog.compile();
        let persisted_scope = Scope {
            languages: spec.scope.languages.clone(),
            countries: spec.scope.countries.clone(),
            tags: spec.scope.tags.clone(),
            catalog: catalog.clone(),
            correlation_id: None,
        };
        let live_scope = Scope {
            correlation_id: Some(correlation_id),
            ..persisted_scope.clone()
        };

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

        Ok((orchestrator, persisted_scope))
    }

    /// Build an [`Orchestrator`] for the anonymize path: reuse
    /// the persisted [`Scope`] from analyze, wire the requested
    /// `policies` + reviewer `overrides` onto every modality's
    /// anonymizer, and skip the analyzer compile (analysis
    /// already happened; only [`Anonymizer`] state matters here).
    ///
    /// Each modality gets an empty [`Analyzer<M>`]; elide's
    /// [`Orchestrator::anonymize_with`] doesn't run recognition
    /// on this path, so an empty analyzer is a zero-cost
    /// placeholder needed only to satisfy `with_modality`'s type
    /// contract.
    ///
    /// The `correlation_id` from the anonymize-time
    /// [`super::Document`] is stamped fresh onto the returned
    /// orchestrator's scope so anonymize-side tracing spans are
    /// distinct from the analyze-side ones.
    ///
    /// [`Scope`]: elide::recognition::Scope
    /// [`Analyzer<M>`]: elide::detection::Analyzer
    pub(super) fn build_anonymize_orchestrator(
        &self,
        scope: &Scope,
        policies: &[Policy],
        overrides: &[(Uuid, PolicyAction)],
        correlation_id: Uuid,
    ) -> Result<Orchestrator<'_>> {
        let live_scope = Scope {
            correlation_id: Some(correlation_id),
            ..scope.clone()
        };
        let catalog = &scope.catalog;

        let text_anon = assemble::<Text, _, _>(
            catalog,
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
                catalog,
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
                catalog,
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
                catalog,
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
/// layer reviewer overrides (so overrides win over policy
/// rules), then attach the policy chain.
///
/// `attach_override` and `attach_policies` are the per-modality
/// bridges into [`crate::anonymizer`]; they know which
/// `ModalityRedactions` field to read and how to build the typed
/// operator. This helper owns the invariant order and the error
/// wrapping so each modality's callsite is one call.
fn assemble<'a, M, O, P>(
    catalog: &LabelCatalog,
    overrides: &[(Uuid, PolicyAction)],
    policies: &'a [Policy],
    attach_override: O,
    attach_policies: P,
) -> Result<Anonymizer<M>>
where
    M: Modality + 'static,
    O: Fn(Anonymizer<M>, Uuid, &PolicyAction) -> std::result::Result<Anonymizer<M>, elide::Error>,
    P: FnOnce(
        Anonymizer<M>,
        std::slice::Iter<'a, Policy>,
    ) -> std::result::Result<Anonymizer<M>, elide::Error>,
{
    let mut anonymizer = Anonymizer::<M>::new().with_catalog(catalog.clone());
    for (id, action) in overrides {
        anonymizer = attach_override(anonymizer, *id, action)?;
    }
    Ok(attach_policies(anonymizer, policies.iter())?)
}
