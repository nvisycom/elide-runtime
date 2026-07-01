//! Compile text-modality rules to elide operators + attach to an
//! [`Anonymizer<Text>`].

use elide::redaction::Anonymizer;
use elide_core::Error;
use elide_core::modality::text::Text;
use nvisy_core::policy::RuleAction;
use nvisy_core::policy::redaction::ModalityRedactions;
use uuid::Uuid;

use super::dispatch::{Target, attach_one_override, attach_policies};
use super::text_op::build_text_op;

/// Attach every text-applicable rule from `policies` onto an
/// already-constructed anonymizer. The apply pipeline calls this
/// after layering per-entity override rules so the overrides
/// keep first-match precedence. Takes an iterator (not a slice)
/// so callers can pre-filter by [`Policy::applies_when`] without
/// cloning the policy set.
///
/// [`Policy::applies_when`]: nvisy_core::policy::Policy::applies_when
pub(crate) fn attach_policies_text<'a>(
    anonymizer: Anonymizer<Text>,
    policies: impl Iterator<Item = &'a nvisy_core::policy::Policy>,
) -> Result<Anonymizer<Text>, Error> {
    attach_policies(anonymizer, policies, compile_one)
}

/// Attach a reviewer override for one entity. The override only
/// applies to `RuleAction::Redact` with a text-modality spec —
/// other action shapes (or non-text overrides) are no-ops at the
/// per-modality dispatch boundary.
pub(crate) fn attach_override_text(
    anonymizer: Anonymizer<Text>,
    entity_id: Uuid,
    action: &RuleAction,
) -> Result<Anonymizer<Text>, Error> {
    attach_one_override(anonymizer, entity_id, action, compile_one)
}

fn compile_one(
    target: Target<'_, Text>,
    redactions: &ModalityRedactions,
) -> Result<Anonymizer<Text>, Error> {
    let Some(spec) = &redactions.text else {
        return Ok(target.passthrough());
    };
    Ok(build_text_op(spec)?.attach_to(target))
}
