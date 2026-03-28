//! LLM provider connection parameters.
//!
//! Provider structs carry API keys, model names, and optional base URLs.
//! The actual rig-core client is constructed lazily when a service is built.

mod authenticated;
mod unauthenticated;

pub use self::authenticated::AuthenticatedProvider;
pub use self::unauthenticated::UnauthenticatedProvider;
