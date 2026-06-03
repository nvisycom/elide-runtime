//! NER-side LLM agent: detection ([`NerAgent`]).
//!
//! [`NerAgent`] implements [`EntityRecognizer<Text>`]. Each call
//! runs the unified detect pass — open-ended entity discovery plus
//! per-hint adjudication when the caller stamps [`Hint<Text>`]s
//! onto [`RecognizerInput::hints`]. It localizes LLM-produced
//! candidates back into byte ranges and emits ready-to-use
//! [`Entity<Text>`] values stamped with the appropriate recognition
//! method.
//!
//! [`UnresolvedCandidatePolicy`] is re-exported here for callers
//! that need to configure how the offset resolver handles
//! candidates it can't uniquely place in the source.
//!
//! [`EntityRecognizer<Text>`]: nvisy_core::EntityRecognizer
//! [`Hint<Text>`]: nvisy_core::Hint
//! [`RecognizerInput::hints`]: nvisy_core::RecognizerInput::hints
//! [`Entity<Text>`]: nvisy_core::entity::Entity

mod detect;

pub use self::detect::{NerAgent, NerCandidate, UnresolvedCandidatePolicy};
