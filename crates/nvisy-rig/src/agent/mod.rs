//! Agent system: base agent, specialized agents, and tool-provider traits.

mod base;
mod context;

pub mod ner;
pub mod ocr;
pub mod cv;
pub mod redactor;

pub(crate) use base::{BaseAgent, BaseAgentBuilder, BaseAgentConfig};
pub(crate) use context::ContextWindow;
