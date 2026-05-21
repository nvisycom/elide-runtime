//! Foundation agent and builder shared by all specialized agents.

mod agent;
mod builder;
mod config;
pub(crate) mod connection;
mod context;
mod metrics;
mod provider;
mod response;

pub use self::agent::AgentConfig;
pub(crate) use self::agent::{Agents, BaseAgent};
pub(crate) use self::builder::BaseAgentBuilder;
pub(crate) use self::config::ALL_TYPES_HINT;
pub use self::config::DetectionConfig;
pub use self::connection::{AuthenticatedProvider, UnauthenticatedProvider};
pub use self::context::ContextWindow;
pub use self::metrics::{UsageStats, UsageTracker};
pub use self::provider::AgentProvider;
pub(crate) use self::response::ResponseParser;
