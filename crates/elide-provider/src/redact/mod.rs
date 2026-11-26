//! Redaction: which entities to hide, and how.
//!
//! Compiles a [`elide_schema::policy::Policy`] set into an
//! [`Anonymizer`] per modality at request time.
//!
//! Mirrors [`crate::detect`], which does the same for the
//! other direction: where recognition finds entities, this hides
//! them. The asymmetry is where the configuration comes
//! from — recognition's is deployment-owned and wired once at
//! startup, redaction's arrives per request as policies.
//!
//! Policy specs are serialisable and modality-agnostic; elide's
//! [`Anonymizer`]`<M>` is a runtime, modality-typed value that
//! drives actual redaction. This module bridges the two: it walks every
//! enabled rule in precedence order, builds the matching elide
//! operator from the spec, and attaches it to the anonymizer with a
//! predicate built from the rule's selector.
//!
//! ## Layout
//!
//! - `text` / `tabular` consume the text-backed redaction specs
//!   (the full elide built-in vocabulary: Remove, Keep, Mask,
//!   Replace, Hash, Pseudonymize, Encrypt: plus the structural
//!   DropRow / DropColumn on tabular).
//! - `image` handles the image specs (Cover, Keep, Blur, Pixelate).
//! - `audio` handles the audio specs (Remove, Keep, Cover, Beep).
//!
//! Each per-modality `compile` entry walks `&[Policy]` in
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
mod dispatch;
mod hidden;
mod image;
mod operator;
mod selector;
mod tabular;
mod target;
mod text;

use elide::entity::LabelCatalog;
use elide::modality::Modality;
use elide::modality::audio::Audio;
use elide::modality::image::Image;
use elide::modality::metadata::Metadata;
use elide::modality::tabular::Tabular;
use elide::modality::text::Text;
use elide::redaction::operators::{Keep, KeyProvider, Remove};
use elide::redaction::{Anonymizer, Operator, Rule};
use elide::{Orchestrator, Result};
use elide_review::HiddenContentPlan;
use elide_schema::policy::Policy;

use self::audio::attach_policies_audio;
pub use self::hidden::hidden_content_policy;
use self::image::attach_policies_image;
use self::operator::text::TextOperatorContext;
use self::tabular::attach_policies_tabular;
use self::text::attach_policies_text;

/// An [`Orchestrator`] that can redact every modality, built from
/// the `policies` this request submitted and the `plan` for its
/// hidden content.
///
/// The mirror of [`analyzers`](crate::detect::analyzers), and
/// the asymmetry is where the configuration comes from:
/// recognition's lineups are deployment-owned and wired once, while
/// redaction's arrive per request as policies.
///
/// Only the anonymizer half is set. Recognition already ran, so
/// `with_anonymizer` leaves each analyzer at its default rather than
/// building ones that never see a document. The caller adds the
/// [`FormatRegistry`](elide::codec::FormatRegistry) and the
/// per-request [`Scope`](elide::recognition::Scope).
///
/// A pick pass at analyze builds the same thing with no `key`: a
/// pick only names the operator that *would* run, so the plan a
/// reviewer sees is the one anonymize applies.
///
/// # Errors
///
/// Returns [`Configuration`](elide::ErrorKind::Configuration) if a
/// policy names an operator that cannot be compiled — an `HmacHash`
/// or `Encrypt` with no `key`, say, or a selector naming a label the
/// catalog does not carry.
pub fn anonymizers(
    catalog: &LabelCatalog,
    policies: &[Policy],
    plan: &HiddenContentPlan,
    key: Option<Arc<dyn KeyProvider>>,
) -> Result<Orchestrator> {
    // Fresh per-request text-operator context. Pseudonym vaults
    // materialise per-policy on first access, so two policies
    // pseudonymising the same entity do not share a surrogate
    // namespace.
    let text_ctx = TextOperatorContext::new(key);

    let text = attach_policies_text(start(catalog, plan), policies.iter(), &text_ctx)?;
    let tabular = attach_policies_tabular(bare(catalog), policies.iter(), &text_ctx)?;
    let image = attach_policies_image(bare(catalog), policies.iter())?;
    let audio = attach_policies_audio(bare(catalog), policies.iter())?;
    // Policies have no metadata arm: a field is hidden content,
    // removed or kept whole by the plan, which keeps what lies inside
    // a kept field. What is left is a finding inside a removed
    // field's value, so it goes with the field.
    let metadata = start::<Metadata>(catalog, plan).with(Rule::fallback(Remove));

    Ok(Orchestrator::new()
        .with_anonymizer::<Text>(text)
        .with_anonymizer::<Tabular>(tabular)
        .with_anonymizer::<Image>(image)
        .with_anonymizer::<Audio>(audio)
        .with_anonymizer::<Metadata>(metadata))
}

/// An anonymizer knowing the request's label vocabulary and the
/// hidden-content plan's two rules, and no policies yet: where a
/// text or metadata anonymizer starts.
///
/// The plan's rules come first, so no policy rule reaches a flagged
/// item.
fn start<M>(catalog: &LabelCatalog, plan: &HiddenContentPlan) -> Anonymizer<M>
where
    M: Modality + 'static,
    Remove: Operator<M>,
    Keep: Operator<M>,
{
    bare(catalog).with_multiple(plan.rules())
}

/// An anonymizer knowing the request's label vocabulary, and no
/// rules yet: where a tabular, image, or audio anonymizer starts.
///
/// Those streams flag no hidden content, so the plan has nothing
/// for them.
fn bare<M: Modality + 'static>(catalog: &LabelCatalog) -> Anonymizer<M> {
    Anonymizer::<M>::new().with_catalog(catalog.clone())
}
