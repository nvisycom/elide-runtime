//! Redaction node configuration.
//!
//! [`Redaction`] runs at **phase 4**, alongside [`GenerateContext`], after
//! fusion has produced a final scored entity list. It applies redaction
//! instructions to the document envelope, replacing or removing detected
//! values, and optionally strips embedded document metadata.
//!
//! [`GenerateContext`]: crate::graph::GenerateContext

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Configuration for the [`Redaction`] graph node.
///
/// Controls which supplementary sanitisation steps are performed in addition
/// to span-level value replacement.
///
/// [`Redaction`]: crate::graph::GraphNodeKind::Redaction
#[derive(Debug, Clone, Default, PartialEq, Eq)]
#[derive(Serialize, Deserialize, JsonSchema)]
pub struct Redaction {
    /// Strip or redact document metadata (EXIF, PDF properties).
    #[serde(default)]
    pub process_metadata: bool,
}
