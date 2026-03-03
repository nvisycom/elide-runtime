//! Signature reference data.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use nvisy_core::math::BoundingBox;
use nvisy_core::path::ContentSource;

/// Reference handwritten signature for verification.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct SignatureData {
    /// Source pointer to the reference signature image.
    pub image_source: ContentSource,
    /// Bounding box of the signature within the image.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub region: Option<BoundingBox>,
    /// Image format hint (e.g. `"png"`, `"jpeg"`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub format: Option<String>,
    /// Identity of the signer this signature belongs to.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub signer_id: Option<String>,
}

impl SignatureData {
    /// Create signature data pointing at a source image.
    pub fn new(image_source: ContentSource) -> Self {
        Self {
            image_source,
            region: None,
            format: None,
            signer_id: None,
        }
    }

    /// Set the signer identity.
    pub fn with_signer_id(mut self, signer_id: impl Into<String>) -> Self {
        self.signer_id = Some(signer_id.into());
        self
    }
}
