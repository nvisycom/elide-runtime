//! [`LlmPrompt`]: how a recognizer's prompt is sourced.

use std::path::PathBuf;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Where a recognizer's Jinja2 prompt template comes from.
///
/// Omitted from an [`LlmRecognizer`](super::LlmRecognizer) means
/// "use elide's default recognition prompt for this modality."
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "source", rename_all = "snake_case")]
#[non_exhaustive]
pub enum LlmPrompt {
    /// Load the Jinja2 template from a file on disk. Resolved
    /// relative to the config file's directory (or absolute if
    /// the path is absolute). File is read at analyzer compile
    /// time.
    File {
        /// Path to the template file. Extensions like `.j2` /
        /// `.jinja2` are conventional but not required.
        path: PathBuf,
    },
    /// Inline template string. Useful for tests and quick
    /// experiments; production deployments usually use `File`.
    Inline {
        /// The Jinja2 template text.
        template: String,
    },
}
