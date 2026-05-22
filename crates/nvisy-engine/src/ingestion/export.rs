//! Export sink configuration.
//!
//! [`ExportFile`] delivers the processed (and optionally redacted)
//! content to one or more destination content objects, applying
//! encryption and compression as requested before writing the bytes
//! out.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use validator::Validate;

use super::{CompressionAlgorithm, EncryptionConfig};

/// Configuration for the [`ExportFile`] config.
///
/// Identifies the destination content objects and specifies any encoding
/// steps that must be applied before the bytes are written out.
///
/// [`ExportFile`]: crate::ingestion::ExportFile
#[derive(Debug, Clone, Default, PartialEq, Eq, Validate)]
#[derive(Serialize, Deserialize, JsonSchema)]
pub struct ExportFile {
    /// Identifiers of content destinations to export to.
    #[serde(default)]
    pub content_ids: Vec<Uuid>,
    /// Encrypt the content before publishing.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub encryption: Option<EncryptionConfig>,
    /// Compress the content before publishing.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub compression: Option<CompressionAlgorithm>,
}
