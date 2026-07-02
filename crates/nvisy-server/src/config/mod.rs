//! Deployment configuration types.
//!
//! Shape only: the deserialisable types [`ServiceRuntime`]
//! expects. Config-source concerns (TOML vs env vs URL) and
//! error reporting live with the consuming binary, not here.
//!
//! ## Layout
//!
//! - [`AppConfig`]: top-level bag. `server`, `analyzer`, `ner`,
//!   `llm`.
//! - [`ServerConfig`]: network binding, lifecycle, nested
//!   observability + middleware sections.
//! - [`MiddlewareConfig`]: body limits, request timeout, CORS.
//! - [`ObservabilityConfig`]: log level filter + output format.
//!
//! Analyzer type comes from
//! [`nvisy_schema::plan::AnalyzerParams`]; NER and LLM come from
//! [`nvisy_core::ner::NerConfig`] and
//! [`nvisy_core::llm::LlmConfig`].
//!
//! [`ServiceRuntime`]: crate::ServiceRuntime

pub mod middleware;
pub mod observability;
mod server;

use nvisy_core::llm::LlmConfig;
use nvisy_core::ner::NerConfig;
use nvisy_schema::plan::AnalyzerParams;
use serde::Deserialize;

pub use self::middleware::{CorsConfig, MiddlewareConfig};
pub use self::observability::{LogFormat, ObservabilityConfig};
pub use self::server::ServerConfig;

/// Resolved top-level configuration.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct AppConfig {
    /// Server, observability, and middleware configuration.
    #[serde(default)]
    pub server: ServerConfig,
    /// Default analyzer spec the server applies to every
    /// request that doesn't override its analyzer fields.
    /// Empty when the `[analyzer]` section is absent; requests
    /// then have to ship a complete spec or get "nothing detects"
    /// semantics.
    #[serde(default)]
    pub analyzer: AnalyzerParams,
    /// NER recognizer lineup. Empty when the `[ner]` section is
    /// absent; requests that set `recognizers.ner = true` will
    /// then fail at compile with a `Validation` error.
    #[serde(default)]
    pub ner: NerConfig,
    /// LLM recognizer lineup + per-provider credentials. Empty
    /// when the `[llm]` section is absent; requests that set
    /// `recognizers.llm = true` will then fail at compile with
    /// a `Validation` error.
    #[serde(default)]
    pub llm: LlmConfig,
}
