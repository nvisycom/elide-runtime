//! Recognition primitives — the rule shapes ([`Regex`] + its
//! [`Variant`]s, [`Dictionary`]), their building blocks ([`Terms`]),
//! and the runtime [`PatternRecognizer`] that compiles them into
//! pooled scanners. Per-rule and per-dictionary `context` keyword
//! lists are harvested by the recognizer at build time into a
//! wrapping `Boosting` layer that applies post-recognition keyword
//! boosts.

mod compiled;
mod dictionary;
mod recognizer;
mod regex;
mod terms;

pub use self::dictionary::{Dictionary, DictionaryBuilder, Scoring};
pub use self::recognizer::{PatternRecognizer, PatternRecognizerBuilder};
pub use self::regex::{Regex, RegexBuilder, Variant, VariantBuilder};
pub use self::terms::{Term, Terms};
