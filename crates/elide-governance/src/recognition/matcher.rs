//! [`CustomMatcher`]: how a caller's own label gets detected.
//!
//! A request introduces labels elide does not ship via [`custom`].
//! That declares the *vocabulary* — what the thing is called — and
//! nothing more: no recognizer looks for it, so the label is
//! scoped, rules target it, and it never matches. A matcher is the
//! other half, saying how to find it.
//!
//! The split mirrors elide's own: a `Label` is identity, a
//! `Regex`/`Dictionary` is detection. Keeping them apart also
//! leaves room for several matchers per label.
//!
//! [`custom`]: super::Recognition::custom

use elide_core::entity::LabelRef;
use elide_core::primitive::Confidence;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// The score a matcher stamps when it does not say.
///
/// Above the shipped patterns' bulk (0.2–0.5) and above
/// [`Confidence::BASELINE`], so a deliberate custom matcher clears
/// the default filter and usually wins an overlap against a
/// generic built-in. Below the validated high-precision patterns
/// (0.95–0.98) and below [`Confidence::MAX`], which is what a
/// *human* assertion carries.
///
/// Reconciliation keeps the higher-confidence entity, so this
/// decides who wins: a matcher that should outrank a Luhn-checked
/// card has to say so explicitly.
fn default_confidence() -> Confidence {
    Confidence::clamped(0.6)
}

/// How to detect one caller-authored label.
///
/// Names a label the same request declares in [`custom`]. A
/// matcher for a shipped built-in is rejected: elide already
/// detects those, and two definitions for one label would race in
/// reconciliation.
///
/// [`custom`]: super::Recognition::custom
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct CustomMatcher {
    /// The label this detects, which the request must declare in
    /// [`custom`](super::Recognition::custom).
    pub label: LabelRef,
    /// Human-readable name, recorded in the audit as the
    /// recognizer that found the entity, so a trail distinguishes
    /// a caller's matcher from a shipped pattern.
    pub name: String,
    /// Score stamped on every match. Defaults to `0.6`.
    ///
    /// Reconciliation keeps the higher-confidence entity when two
    /// detections overlap, so this is what decides whether a
    /// custom match beats a built-in one covering the same span.
    #[serde(default = "default_confidence")]
    #[schemars(with = "f32")]
    pub confidence: Confidence,
    /// What to match on.
    #[serde(flatten)]
    pub match_on: MatchOn,
}

/// What a [`CustomMatcher`] scans for.
///
/// An enum rather than two optional fields, so "neither supplied"
/// cannot be expressed: a matcher that matches nothing would be
/// accepted, scoped, and silently never fire.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum MatchOn {
    /// A regular expression over the decoded text.
    ///
    /// Compiled per request against a size limit. The `regex`
    /// crate does not backtrack, so a pathological pattern costs
    /// compile time rather than match time.
    Pattern {
        /// The expression, in `regex` crate syntax.
        pattern: String,
    },
    /// A literal term list, matched whole-word.
    ///
    /// For the vocabulary a regex describes badly: client names,
    /// internal codenames, a fixed roster of identifiers.
    Terms {
        /// The literals to scan for.
        terms: Vec<String>,
    },
}
