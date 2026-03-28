//! Foundation agent and builder shared by all specialized agents.

mod agent;
mod builder;
mod context;
mod detection;
mod provider;
mod response;

pub use self::agent::AgentConfig;
pub(crate) use self::agent::{Agents, BaseAgent};
pub(crate) use self::builder::BaseAgentBuilder;
pub use self::context::ContextWindow;
pub(crate) use self::detection::ALL_TYPES_HINT;
pub use self::detection::{DetectionConfig, DetectionRequest, DetectionResponse};
pub use self::provider::AgentProvider;
pub(crate) use self::response::ResponseParser;
