//! Shared LLM provider connection parameters.
//!
//! Both the LLM-completion path ([`backend::rig`]) and the speech-to-text
//! path ([`audio::stt`]) need the same set of provider connection
//! parameters (API key, model, optional base URL). This module owns
//! the single definition of those parameter structs plus the
//! `LlmProvider` enum that the rig backend consumes.
//!
//! [`backend::rig`]: crate::backend
//! [`audio::stt`]: crate::audio::stt

mod authenticated;
mod llm;
mod unauthenticated;

pub use self::authenticated::AuthenticatedProvider;
pub use self::llm::LlmProvider;
pub use self::unauthenticated::UnauthenticatedProvider;
