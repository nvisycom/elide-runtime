//! Generate context node configuration.
//!
//! [`GenerateContext`] runs at **phase 4**, alongside [`Redaction`], after
//! detection and deduplication are complete. It synthesises a new context entry from
//! the detection results and the processed document envelope, optionally
//! enriching it with summarisation, translation, and audit records.
//!
//! [`Redaction`]: crate::graph::Redaction

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Configuration for the [`GenerateContext`] graph node.
///
/// Controls which supplementary outputs are generated alongside the base
/// context record produced from detection results.
///
/// [`GenerateContext`]: crate::graph::GraphNodeKind::GenerateContext
#[derive(Debug, Clone, Default, PartialEq, Eq)]
#[derive(Serialize, Deserialize, JsonSchema)]
pub struct GenerateContext {
    /// Include a span-level summary in the generated context.
    #[serde(default)]
    pub summarization: bool,
    /// Include translated spans in the generated context.
    #[serde(default)]
    pub translation: bool,
    /// Include an audit record in the generated context.
    #[serde(default)]
    pub audit: bool,
}
