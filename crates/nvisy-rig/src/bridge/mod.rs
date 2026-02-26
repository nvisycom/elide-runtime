//! Prompt construction and LLM response parsing.
//!
//! [`PromptBuilder`] assembles user prompts with entity-kind filters and
//! confidence thresholds. [`ResponseParser`] extracts and deserializes
//! text from rig-core completion responses. [`EntityParser`] converts raw
//! JSON dicts into [`Entity`](nvisy_ontology::entity::Entity) values.

mod prompt;
mod response;

pub use prompt::PromptBuilder;
pub use response::{EntityParser, ResponseParser};
