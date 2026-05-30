//! NER-side LLM agents: detection ([`NerAgent`]) and verification
//! ([`NerVerifyAgent`]).
//!
//! [`NerAgent`] runs the unified detect pass — open-ended entity
//! discovery plus per-hint adjudication when the caller passes
//! [`hints`]. It localizes LLM-produced candidates back into byte
//! ranges and emits ready-to-use [`Entity<Text>`] values stamped
//! with the appropriate recognition method.
//!
//! [`NerVerifyAgent`] runs the optional whole-audit verify pass:
//! given the merged entity set from every recognizer, it asks the
//! LLM to confirm, reject, or adjust each entity and returns the
//! survivors.
//!
//! [`UnresolvedCandidatePolicy`] is re-exported here for callers
//! that need to configure how the offset resolver handles
//! candidates it can't uniquely place in the source.
//!
//! [`hints`]: crate::agent::LlmNerContext::hints
//! [`Entity<Text>`]: nvisy_ontology::entity::Entity

mod detect;
mod verify;

pub use self::detect::{NerAgent, NerCandidate, UnresolvedCandidatePolicy};
pub use self::verify::NerVerifyAgent;
