//! Recognition primitives — the rule shapes ([`Regex`],
//! [`Dictionary`]), their building blocks ([`Terms`] plus
//! [`Context`](nvisy_core::context::Context) from `nvisy-core`),
//! the [`PatternRegistry`] that bundles them, and the runtime
//! [`PatternRecognizer`] that compiles them into pooled scanners.

mod dictionary;
mod recognizer;
mod regex_rule;
mod registry;
mod terms;

pub use self::dictionary::{Dictionary, DictionaryBuilder};
pub use self::recognizer::{PatternRecognizer, PatternRecognizerBuilder};
pub use self::regex_rule::{Regex, RegexBuilder};
pub use self::registry::PatternRegistry;
pub use self::terms::Terms;
