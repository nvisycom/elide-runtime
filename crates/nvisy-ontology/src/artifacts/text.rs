//! Text-modality artifacts.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Artifacts produced during processing of text content.
#[derive(Debug, Clone, Default, PartialEq)]
#[derive(Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct TextArtifacts {}
