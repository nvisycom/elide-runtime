//! [`LeakProfile`]: how much an operator's output leaks about the
//! original it replaced.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// What a redacted output leaks about the original it replaced.
///
/// Variants are ordered from most-leaky to least-leaky, so
/// `Recoverable < Partial < Irrecoverable`. Surfaced through
/// [`Anonymizer::leak_profile`] for policy authoring and audit
/// reporting.
///
/// [`Anonymizer::leak_profile`]: super::Anonymizer::leak_profile
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[derive(Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum LeakProfile {
    /// The original value is recoverable from the output given the
    /// right metadata (encryption key, token vault, pseudonym map,
    /// or the candidate entity list against a hash).
    Recoverable,
    /// The original value is gone, but observable shape leaks:
    /// position, length, bounding box, cell coordinates, or a known
    /// silence on the timeline.
    Partial,
    /// No trace of the original value or its shape remains in the
    /// output.
    Irrecoverable,
}
