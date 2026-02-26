//! Foundation agent and builder shared by all specialized agents.

mod agent;
mod builder;

pub use agent::BaseAgentConfig;
pub(crate) use agent::{Agents, BaseAgent};
pub(crate) use builder::BaseAgentBuilder;
