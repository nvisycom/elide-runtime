//! Post-recognition keyword-boost enhancement, shared across every
//! [`EntityRecognizer<Text>`].
//!
//! The enhancer takes a slice of detected entities plus the source
//! text and the shared `RecognizerInput::artifacts` `TypeMap`, and for each
//! entity:
//!
//! 1. Pulls the source recognizer's name from the entity's first
//!    `TrailStep` provenance.
//! 2. Looks the name up in a [`ContextRegistry`] to find the
//!    declared keyword [`Context`].
//! 3. Walks the surrounding window (token-based when `Tokens` is
//!    present in the artifact map, substring-based otherwise) and
//!    asks the configured [`KeywordMatcher`] whether any keyword
//!    fired.
//! 4. Applies the configured boost (or the per-entity override),
//!    capped at `1.0`, and appends a `Refinement` step to the
//!    trail.
//!
//! The registry shape — `name → Context` — is the same pattern
//! Presidio uses: each recognizer (or each rule within a
//! recognizer) registers a *source name* and a keyword list, and
//! the enhancer dispatches on the name carried in the entity's
//! provenance. Per-rule contexts for patterns (`Regex.context`,
//! `Dictionary.context` in `nvisy-pattern`) and per-recognizer
//! contexts for NER (`NerRecognizer.default_context` in
//! `nvisy-ner`) plug into the same registry.
//!
//! [`EntityRecognizer<Text>`]: crate::EntityRecognizer

mod declaration;
mod enhancer;
mod matcher;
mod registry;
mod tokens;

pub use self::declaration::Context;
pub use self::enhancer::{ContextEnhancer, ContextEnhancerBuilder, ContextEnhancerBuilderError};
pub use self::matcher::{KeywordMatcher, LemmaMatcher, SubstringMatcher};
pub use self::registry::ContextRegistry;
pub use self::tokens::{Token, Tokens};
