//! Output types produced by [`NlpEngine::analyze`].
//!
//! - [`NlpArtifacts`] — the per-call composite result returned by the
//!   engine.
//! - [`Token`] — a single token produced by a [`Tokenizer`].
//!
//! [`NlpEngine::analyze`]: crate::engine::NlpEngine::analyze
//! [`Tokenizer`]: crate::tokenizer::Tokenizer

mod nlp_artifacts;
mod token;

pub use self::nlp_artifacts::NlpArtifacts;
pub use self::token::Token;
