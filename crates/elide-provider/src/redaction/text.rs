//! Compile text-modality rules to elide operators + attach to an
//! [`Anonymizer<Text>`].

use elide::Result;
use elide::modality::text::Text;
use elide::redaction::Anonymizer;
use elide_governance::PolicyDefinition;
use elide_governance::redaction::{ModalityRedactions, TextRedaction};

use super::compile::{Target, attach_policies};
use super::operator::text::{TextOperatorContext, compile_and_attach};

/// Attach every text-applicable rule from `policies` onto an
/// already-constructed anonymizer. Takes an iterator (not a slice)
/// so callers can pre-filter by [`PolicyDefinition::when`] without
/// cloning the policy set.
///
/// [`PolicyDefinition::when`]: elide_governance::PolicyDefinition::when
pub(crate) fn attach_policies_text<'a>(
    anonymizer: Anonymizer<Text>,
    policies: impl Iterator<Item = &'a PolicyDefinition> + Clone,
    ctx: &TextOperatorContext,
) -> Result<Anonymizer<Text>> {
    attach_policies(anonymizer, policies, |target, redactions| {
        compile_one(target, redactions, ctx)
    })
}

fn compile_one(
    target: Target<'_, Text>,
    redactions: &ModalityRedactions,
    ctx: &TextOperatorContext,
) -> Result<Anonymizer<Text>> {
    match &redactions.text {
        None => Ok(target.passthrough()),
        Some(spec) => compile_spec(target, spec, ctx),
    }
}

fn compile_spec(
    target: Target<'_, Text>,
    spec: &TextRedaction,
    ctx: &TextOperatorContext,
) -> Result<Anonymizer<Text>> {
    compile_and_attach(spec, ctx, target)
}
