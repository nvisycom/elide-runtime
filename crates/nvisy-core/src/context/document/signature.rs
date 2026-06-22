//! Signature reference data.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::ContentSource;
use crate::schema::BoundingBoxSchema;

/// Reference handwritten signature for verification.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct SignatureData {
    /// Source pointer to the reference signature image.
    pub image_source: ContentSource,
    /// Bounding box of the signature within the image.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub region: Option<BoundingBoxSchema>,
    /// Identity of the signer this signature belongs to.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub signer_id: Option<String>,
    /// Algorithm used for signature verification.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub algorithm: Option<String>,
}

impl SignatureData {
    /// Create signature data pointing at a source image.
    pub fn new(image_source: ContentSource) -> Self {
        Self {
            image_source,
            region: None,
            signer_id: None,
            algorithm: None,
        }
    }

    /// Set the signer identity.
    pub fn with_signer_id(mut self, signer_id: impl Into<String>) -> Self {
        self.signer_id = Some(signer_id.into());
        self
    }

    /// Set the verification algorithm.
    pub fn with_algorithm(mut self, algorithm: impl Into<String>) -> Self {
        self.algorithm = Some(algorithm.into());
        self
    }
}
