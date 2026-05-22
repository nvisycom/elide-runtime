//! NER-side LLM agents: detection ([`NerAgent`]) and verification
//! ([`NerVerifyAgent`]).
//!
//! [`NerAgent`] asks an LLM for entity candidates given text (plus
//! accumulated known entities for cross-call coreference).
//! [`NerVerifyAgent`] localizes those candidates into byte ranges
//! and optionally LLM-refines them.

mod detect;
mod verify;

pub use self::detect::{KnownNerEntity, NerAgent, NerCandidate, NerContext};
pub use self::verify::{NerVerifyAgent, UnresolvedCandidatePolicy};
