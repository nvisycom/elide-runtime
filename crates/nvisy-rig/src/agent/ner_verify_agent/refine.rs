//! LLM refinement pass for localized NER candidates.
//!
//! Sends the localized candidates plus a snippet of surrounding
//! text (built by [`NerVerifyAgentPromptBuilder`]) to the LLM, asks
//! it to vote keep/correct/reject per entry, and folds the
//! verdicts back into the localized list.
//!
//! [`NerVerifyAgentPromptBuilder`]: super::prompt::NerVerifyAgentPromptBuilder

use std::collections::HashMap;

use nvisy_core::Result;

use super::localize::LocalizedCandidate;
use super::prompt::NerVerifyAgentPromptBuilder;
use crate::agent::base::{BaseAgent, VerificationOutput, VerificationStatus};

/// Run the refinement pass over a slice of localized candidates.
///
/// Returns the surviving (kept or corrected) candidates. Rejected
/// entries are dropped; corrected entries have their fields
/// updated in place.
pub(super) async fn refine_localized(
    agent: &BaseAgent,
    text: &str,
    localized: Vec<LocalizedCandidate>,
) -> Result<Vec<LocalizedCandidate>> {
    if localized.is_empty() {
        return Ok(localized);
    }

    let prompt = NerVerifyAgentPromptBuilder::new(text, &localized).build();
    let output: VerificationOutput = agent
        .prompt_structured_raw(&prompt)
        .await
        .map_err(crate::error::convert)?;

    Ok(apply_verdicts(localized, output))
}

fn apply_verdicts(
    mut localized: Vec<LocalizedCandidate>,
    output: VerificationOutput,
) -> Vec<LocalizedCandidate> {
    let verdicts: HashMap<usize, _> = output.entities.into_iter().map(|v| (v.id, v)).collect();

    let mut out = Vec::with_capacity(localized.len());
    for (i, mut l) in localized.drain(..).enumerate() {
        match verdicts.get(&i) {
            None => out.push(l), // implicit confirm
            Some(v) => match v.status {
                VerificationStatus::Rejected => {} // drop
                VerificationStatus::Corrected => {
                    if let Some(ref new_value) = v.value {
                        l.candidate.value = new_value.clone();
                    }
                    if v.category.is_some() {
                        l.candidate.category = v.category;
                    }
                    if v.entity_type.is_some() {
                        l.candidate.entity_type = v.entity_type;
                    }
                    // Update confidence to the verifier's (more
                    // recent) judgement.
                    l.candidate.confidence = Some(v.confidence.get());
                    out.push(l);
                }
            },
        }
    }
    out
}
