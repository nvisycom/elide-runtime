//! Build a per-request [`Orchestrator`] from an [`AnalyzerParams`]
//! plus the resolved policy + override sets.
//!
//! One pipeline per modality, all wired against a single
//! [`elide::recognition::Scope`] (the request's caller-asserted
//! scope + the compiled label catalog + a server-minted
//! correlation id). The orchestrator is built fresh per call —
//! it's a small map of trait objects keyed by modality `TypeId`,
//! cheap to construct, and the per-call shape lets us re-resolve
//! policies and scope per document at apply time without
//! mutating a shared anonymizer.

use elide::Orchestrator;
use elide::codec::FormatRegistry;
use elide::redaction::Anonymizer;
use elide_core::modality::audio::Audio;
use elide_core::modality::image::Image;
use elide_core::modality::tabular::Tabular;
use elide_core::modality::text::Text;
use nvisy_core::plan::AnalyzerParams;
use nvisy_core::policy::{Policy, RuleAction};
use nvisy_core::{Error, Result};
use uuid::Uuid;

use super::analyzer::{AnalyzerCompile, LabelCatalogCompile};
use super::anonymizer::{
    attach_override_audio, attach_override_image, attach_override_tabular, attach_override_text,
    attach_policies_audio, attach_policies_image, attach_policies_tabular, attach_policies_text,
};

const COMPONENT: &str = "engine::orchestrator";

/// Build an [`Orchestrator`] with one pipeline per modality and a
/// request-scoped [`Scope`].
///
/// `policies` is the resolved policy set (empty during analyze).
/// `overrides` are layered onto every modality's anonymizer ahead
/// of the policy chain — entity ids are globally unique across
/// body and parts, so an override matches in exactly one pipeline
/// (the one whose recognized entities include that id) and is a
/// no-op everywhere else.
///
/// [`Scope`]: elide::recognition::Scope
pub(super) fn build<'a>(
    formats: &'a FormatRegistry,
    spec: &AnalyzerParams,
    policies: &[Policy],
    overrides: &[(Uuid, RuleAction)],
    correlation_id: Uuid,
) -> Result<Orchestrator<'a>> {
    let catalog = spec.scope.label_catalog.compile();
    // Assemble the orchestrator's `Scope` from the three wire
    // knobs on `AnalyzerParams` + the caller-supplied
    // `correlation_id` + the resolved catalog. The catalog has
    // exactly one route (LabelCatalogParams); the correlation id
    // is server-minted (typically the run id) and never appears
    // on the wire shape.
    let scope = elide::recognition::Scope {
        languages: spec.scope.languages.clone(),
        countries: spec.scope.countries.clone(),
        labels: spec.scope.labels.clone(),
        catalog: catalog.clone(),
        correlation_id: Some(correlation_id),
    };

    let text_analyzer = spec.compile_text().map_err(compile_err)?;
    let tabular_analyzer = spec.compile_tabular().map_err(compile_err)?;
    let image_analyzer = spec.compile_image().map_err(compile_err)?;
    let audio_analyzer = spec.compile_audio().map_err(compile_err)?;

    // Build each modality's anonymizer fresh: start with the
    // catalog (so `with_tag` / `with_catalog_predicate` see label
    // tags), layer reviewer overrides, then attach the policy
    // chain so policy rules sit behind the overrides.
    let mut text_anonymizer = Anonymizer::<Text>::new().with_catalog(catalog.clone());
    for (id, action) in overrides {
        text_anonymizer =
            attach_override_text(text_anonymizer, *id, action).map_err(compile_err)?;
    }
    let text_anonymizer =
        attach_policies_text(text_anonymizer, policies.iter()).map_err(compile_err)?;

    let mut tabular_anonymizer = Anonymizer::<Tabular>::new().with_catalog(catalog.clone());
    for (id, action) in overrides {
        tabular_anonymizer =
            attach_override_tabular(tabular_anonymizer, *id, action).map_err(compile_err)?;
    }
    let tabular_anonymizer =
        attach_policies_tabular(tabular_anonymizer, policies.iter()).map_err(compile_err)?;

    let mut image_anonymizer = Anonymizer::<Image>::new().with_catalog(catalog.clone());
    for (id, action) in overrides {
        image_anonymizer =
            attach_override_image(image_anonymizer, *id, action).map_err(compile_err)?;
    }
    let image_anonymizer =
        attach_policies_image(image_anonymizer, policies.iter()).map_err(compile_err)?;

    let mut audio_anonymizer = Anonymizer::<Audio>::new().with_catalog(catalog);
    for (id, action) in overrides {
        audio_anonymizer =
            attach_override_audio(audio_anonymizer, *id, action).map_err(compile_err)?;
    }
    let audio_anonymizer =
        attach_policies_audio(audio_anonymizer, policies.iter()).map_err(compile_err)?;

    Ok(Orchestrator::new(formats)
        .with_scope(scope)
        .with_modality::<Text>(text_analyzer, text_anonymizer)
        .with_modality::<Tabular>(tabular_analyzer, tabular_anonymizer)
        .with_modality::<Image>(image_analyzer, image_anonymizer)
        .with_modality::<Audio>(audio_analyzer, audio_anonymizer))
}

/// Translate an `elide::Error` from a `compile_*` call into the
/// runtime's error type. Compile failures are caller-driven (e.g.
/// unsupported recognizer for the modality), so they map to
/// [`Validation`].
///
/// [`Validation`]: nvisy_core::ErrorKind::Validation
fn compile_err(err: elide::Error) -> Error {
    Error::validation(format!("orchestrator compile failed: {err}"), COMPONENT).with_source(err)
}
