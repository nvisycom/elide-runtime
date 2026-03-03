//! Foundation agent and builder shared by all specialized agents.

mod agent;
mod builder;
mod context;
mod detection;
mod provider;
mod response;

pub use agent::AgentConfig;
pub(crate) use agent::{Agents, BaseAgent};
pub(crate) use builder::BaseAgentBuilder;
pub use context::ContextWindow;
pub use provider::AgentProvider;

pub use detection::{DetectionConfig, DetectionRequest, DetectionResponse};
pub(crate) use detection::ALL_TYPES_HINT;
pub(crate) use response::ResponseParser;
