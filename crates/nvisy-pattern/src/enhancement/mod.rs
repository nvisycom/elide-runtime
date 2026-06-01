//! Post-recognition keyword-boost enhancement.
//!
//! [`ContextEnhancer`] takes recognizer output plus the original
//! text, looks each entity's originating regex rule or dictionary up
//! by name in a [`PatternRegistry`](crate::recognition::PatternRegistry),
//! and applies a confidence boost when any of the rule's declared
//! keywords appear in the window around the match.
//!
//! Keyword detection is delegated to a [`KeywordMatcher`] strategy
//! — the default ships [`SubstringMatcher`], but the trait is open
//! for custom impls.

mod context_enhancer;
mod keyword_matcher;

pub use self::context_enhancer::{
    ContextEnhancer, ContextEnhancerBuilder, ContextEnhancerBuilderError,
};
pub use self::keyword_matcher::{KeywordMatcher, SubstringMatcher};
