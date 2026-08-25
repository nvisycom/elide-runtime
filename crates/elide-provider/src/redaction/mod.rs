//! Redaction: which entities to hide, and how.
//!
//! Compiles a [`elide_governance::PolicyDefinition`] set into an
//! [`Anonymizer`] per modality at request time.
//!
//! Mirrors [`crate::recognition`], which does the same for the
//! other direction: where recognition finds entities, this hides
//! them. The asymmetry is where the configuration comes
//! from — recognition's is deployment-owned and wired once at
//! startup, redaction's arrives per request as policies.
//!
//! PolicyDefinition specs are serialisable and modality-agnostic; elide's
//! [`Anonymizer`]`<M>` is a runtime, modality-typed value that
//! drives actual redaction. This module bridges the two: it walks every
//! enabled rule in precedence order, builds the matching elide
//! operator from the spec, and attaches it to the anonymizer with a
//! predicate built from the rule's selector.
//!
//! ## Layout
//!
//! - `text` / `tabular` consume the text-backed redaction specs
//!   (the full elide built-in vocabulary: Erase, Keep, Mask,
//!   Replace, Hash, Pseudonymize, Encrypt: plus the structural
//!   DropRow / DropColumn on tabular).
//! - `image` handles the image specs (Erase, Keep, Blur, Pixelate,
//!   Blackbox).
//! - `audio` handles the audio specs (Erase, Keep, Silence, Beep).
//!
//! Each per-modality `compile` entry walks `&[PolicyDefinition]` in
//! precedence order; within each policy, rules are tried in
//! declared order; the first matching rule's operator wins. A
//! policy's `fallback`, if Redact with that modality's arm set,
//! becomes the anonymizer's catch-all.
//!
//! ## Audit decoration
//!
//! Each compiled operator will be wrapped in a thin decorator that
//! stamps the policy/rule attribution onto the audit when the
//! operator runs. The decoration lives outside the per-modality
//! compile helpers: they assemble naked operators today; the audit
//! pass wraps them in a follow-up.
//!
//! [`Anonymizer`]: elide::redaction::Anonymizer

use std::sync::Arc;

mod audio;
mod compile;
mod image;
mod operator;
mod tabular;
mod text;

use elide::entity::LabelCatalog;
use elide::modality::Modality;
use elide::modality::audio::Audio;
use elide::modality::image::Image;
use elide::modality::tabular::Tabular;
use elide::modality::text::Text;
use elide::redaction::Anonymizer;
use elide::redaction::operators::KeyProvider;
use elide::{Orchestrator, Result};
use elide_governance::PolicyDefinition;
use elide_governance::modality::RedactableModality;
use uuid::Uuid;

use self::audio::{attach_override_audio, attach_policies_audio};
use self::image::{attach_override_image, attach_policies_image};
use self::operator::text::TextOperatorContext;
use self::tabular::{attach_override_tabular, attach_policies_tabular};
use self::text::{attach_override_text, attach_policies_text};
use crate::{Override, Overrides};

/// An [`Orchestrator`] that can redact any of the four modalities,
/// built from the `policies` this request submitted plus whatever
/// its reviewer overrode.
///
/// The mirror of [`analyzers`](crate::recognition::analyzers), and
/// the asymmetry is where the configuration comes from:
/// recognition's lineups are deployment-owned and wired once, while
/// redaction's arrive per request as policies.
///
/// Overrides attach ahead of the policy rules on every modality.
/// elide's anonymizer is first-match, so that ordering *is* reviewer
/// precedence: attach them after and a reviewer's choice is silently
/// ignored.
///
/// Only the anonymizer half is set. Recognition already ran, so
/// `with_anonymizer` leaves each analyzer at its default rather than
/// building four that never see a document. The caller adds the
/// [`FormatRegistry`](elide::codec::FormatRegistry) and the
/// per-request [`Scope`](elide::recognition::Scope).
///
/// # Errors
///
/// Returns [`Configuration`](elide::ErrorKind::Configuration) if a
/// policy names an operator that cannot be compiled — an `HmacHash`
/// or `Encrypt` with no `key`, say, or a selector naming a label the
/// catalog does not carry.
pub fn anonymizers(
    catalog: &LabelCatalog,
    policies: &[PolicyDefinition],
    overrides: &Overrides,
    key: Option<Arc<dyn KeyProvider>>,
) -> Result<Orchestrator> {
    // Fresh per-request text-operator context. Pseudonym vaults
    // materialise per-policy on first access, so two policies
    // pseudonymising the same entity do not share a surrogate
    // namespace. Overrides and policy rules compile against the
    // *same* context, so a reviewer's `Pseudonymize` draws from the
    // vault its policy's other rules use.
    let text_ctx = TextOperatorContext::new(key);

    let text = attach_overrides(
        empty_anonymizer::<Text>(catalog),
        &overrides.text,
        |a, id, policy, action| attach_override_text(a, id, policy, action, &text_ctx),
    )?;
    let text = attach_policies_text(text, policies.iter(), &text_ctx)?;

    let tabular = attach_overrides(
        empty_anonymizer::<Tabular>(catalog),
        &overrides.tabular,
        |a, id, policy, action| attach_override_tabular(a, id, policy, action, &text_ctx),
    )?;
    let tabular = attach_policies_tabular(tabular, policies.iter(), &text_ctx)?;

    let image = attach_overrides(
        empty_anonymizer::<Image>(catalog),
        &overrides.image,
        attach_override_image,
    )?;
    let image = attach_policies_image(image, policies.iter())?;

    let audio = attach_overrides(
        empty_anonymizer::<Audio>(catalog),
        &overrides.audio,
        attach_override_audio,
    )?;
    let audio = attach_policies_audio(audio, policies.iter())?;

    Ok(Orchestrator::new()
        .with_anonymizer::<Text>(text)
        .with_anonymizer::<Tabular>(tabular)
        .with_anonymizer::<Image>(image)
        .with_anonymizer::<Audio>(audio))
}

/// The four anonymizers a pick pass runs through, compiled from
/// `policies` alone.
///
/// What [`anonymizers`] builds, minus the overrides and the key: a
/// pick only names the operator that *would* run, and none exist to
/// override at analyze time. elide still compiles the operator to
/// reach its name, so a policy naming `HmacHash`/`Encrypt` fails
/// here rather than recording a keyless pick — which is why the
/// analyze path tolerates this failing.
///
/// # Errors
///
/// Returns [`Configuration`](elide::ErrorKind::Configuration) if a
/// policy's operators cannot be compiled.
pub fn pickers(catalog: &LabelCatalog, policies: &[PolicyDefinition]) -> Result<Pickers> {
    let text_ctx = TextOperatorContext::new(None);
    Ok(Pickers {
        text: attach_policies_text(empty_anonymizer(catalog), policies.iter(), &text_ctx)?,
        tabular: attach_policies_tabular(empty_anonymizer(catalog), policies.iter(), &text_ctx)?,
        image: attach_policies_image(empty_anonymizer(catalog), policies.iter())?,
        audio: attach_policies_audio(empty_anonymizer(catalog), policies.iter())?,
    })
}

/// One anonymizer per modality, for recording what each entity's
/// operator *would* be without applying anything.
pub struct Pickers {
    /// The text anonymizer.
    pub text: Anonymizer<Text>,
    /// The tabular anonymizer.
    pub tabular: Anonymizer<Tabular>,
    /// The image anonymizer.
    pub image: Anonymizer<Image>,
    /// The audio anonymizer.
    pub audio: Anonymizer<Audio>,
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
/// Call before attaching the policy rules: elide's anonymizer is
/// first-match, so overrides attached ahead of the rules *are* the
/// precedence.
fn attach_overrides<M, F>(
    mut anonymizer: Anonymizer<M>,
    overrides: &[Override<M>],
    attach_one: F,
) -> Result<Anonymizer<M>>
where
    M: RedactableModality + 'static,
    F: Fn(Anonymizer<M>, Uuid, Uuid, &M::Redaction) -> Result<Anonymizer<M>>,
{
    for over in overrides {
        anonymizer = attach_one(anonymizer, over.entity_id, over.policy_id, &over.action)?;
    }
    Ok(anonymizer)
}
