//! Foundation agent and builder shared by all specialized agents.

mod agent;
mod builder;
pub(crate) mod connection;
mod context;
mod detection;
mod metrics;
mod provider;
mod response;
mod verification;

pub use self::agent::AgentConfig;
pub(crate) use self::agent::{Agents, BaseAgent};
pub(crate) use self::builder::BaseAgentBuilder;
#[cfg(any(
    feature = "openai-gpt",
    feature = "anthropic-claude",
    feature = "google-gemini",
    feature = "openai-whisper",
    feature = "openai-tts",
))]
pub use self::connection::AuthenticatedProvider;
pub use self::connection::UnauthenticatedProvider;
pub use self::context::ContextWindow;
pub(crate) use self::detection::ALL_TYPES_HINT;
pub use self::detection::DetectionConfig;
pub use self::metrics::{UsageStats, UsageTracker};
pub use self::provider::AgentProvider;
pub(crate) use self::response::ResponseParser;
pub use self::verification::{VerificationOutput, VerificationStatus, VerifiedEntity};
