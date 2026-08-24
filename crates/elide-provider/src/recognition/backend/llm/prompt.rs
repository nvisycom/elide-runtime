//! [`LlmPrompt`]: how a recognizer's prompt is sourced.

use std::path::PathBuf;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Where a recognizer's Jinja2 prompt template comes from.
///
/// Omitted from an [`LlmRecognizerConfig`] means "use elide's default
/// recognition prompt for this modality."
///
/// [`LlmRecognizerConfig`]: super::LlmRecognizerConfig
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "source", rename_all = "snake_case")]
pub enum LlmPrompt {
    /// Load the Jinja2 template from a file on disk. The path is
    /// passed through as written and read at analyzer compile
    /// time, so a relative path resolves against the *process*
    /// working directory, not the config file's directory. Deploy
    /// with absolute paths unless the host pins its own cwd.
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
