//! Bridge between rig-core and the detection service.
//!
//! Prompt building and response parsing utilities.

mod prompt;
mod response;

pub use prompt::PromptBuilder;
pub use response::{EntityParser, ResponseParser};
