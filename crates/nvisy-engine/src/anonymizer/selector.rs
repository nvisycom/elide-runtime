//! Wire a [`nvisy_core::policy::EntitySelector`] onto an elide
//! [`Anonymizer`] as a label / tag / predicate rule.
//!
//! Single-label and single-tag selectors with no confidence
//! threshold fast-path to elide's [`Anonymizer::with_label`] /
//! [`Anonymizer::with_tag`]; anything more general (combined fields,
//! threshold) wires as [`Anonymizer::with_predicate`].
//!
//! The predicate the closure builds *cannot reach the anonymizer's
//! own catalog*, so tag matching inside the closure is intentionally
//! omitted; tag-heavy selectors must single out via `with_tag` or
//! decompose at the engine layer. This is flagged here so the
//! follow-up that introduces multi-tag predicates knows where the
//! seam is.
//!
//! [`Anonymizer`]: elide::Anonymizer
//! [`Anonymizer::with_label`]: elide::Anonymizer::with_label
//! [`Anonymizer::with_tag`]: elide::Anonymizer::with_tag
//! [`Anonymizer::with_predicate`]: elide::Anonymizer::with_predicate

use elide::Anonymizer;
use elide_core::entity::{Entity, LabelRef};
use elide_core::modality::Modality;
use elide_core::redaction::{Attribution, Operator};
use nvisy_core::policy::{EntitySelector, Policy, PolicyRule};

/// Build an [`Attribution`] for the rule that fired. The wire shape
/// is `{policy.name}#{rule.name}` so an audit reader can split on
/// `#` to recover both halves. `reason` stays `None` today — policy
/// rules carry no human-readable rationale field yet.
pub(super) fn rule_attribution(policy: &Policy, rule: &PolicyRule) -> Attribution {
    Attribution::new(format!("{}#{}", policy.name, rule.name))
}

/// Build an [`Attribution`] for a policy's `default_action`
/// fallback (no rule name).
pub(super) fn default_attribution(policy: &Policy) -> Attribution {
    Attribution::new(format!("{}#<default>", policy.name))
}

/// Attach `operator` to `anonymizer` under the rule the selector
/// expresses, stamping the rule's `attribution` onto the elide
/// rule so every redaction it drives carries `policy_id` + `reason`
/// on its provenance event.
pub(super) fn attach<M, O>(
    anonymizer: Anonymizer<M>,
    selector: &EntitySelector,
    operator: O,
    attribution: Attribution,
) -> Anonymizer<M>
where
    M: Modality,
    O: Operator<M> + Clone + 'static,
{
    let no_threshold = selector.confidence_threshold.is_none();
    let anonymizer = match (selector.labels.as_slice(), selector.tags.as_slice()) {
        ([single], [], ) if no_threshold => {
            anonymizer.with_label(LabelRef::new(single.clone()), operator)
        }
        ([], [single], ) if no_threshold => anonymizer.with_tag(single.clone(), operator),
        _ => anonymizer.with_predicate(predicate::<M>(selector.clone()), operator),
    };
    anonymizer.because(attribution)
}

/// Build a predicate over `entity` from `selector`: confidence
/// threshold and exact-label membership only — tag matching is not
/// reachable here (see module doc).
fn predicate<M>(
    selector: EntitySelector,
) -> impl Fn(&Entity<M>) -> bool + Send + Sync + 'static
where
    M: Modality,
{
    let labels: Vec<LabelRef> = selector.labels.into_iter().map(LabelRef::new).collect();
    let threshold = selector.confidence_threshold;
    move |entity| {
        if let Some(min) = threshold
            && f32::from(entity.confidence) < min
        {
            return false;
        }
        if !labels.is_empty() && !labels.iter().any(|l| l == &entity.label) {
            return false;
        }
        true
    }
}
