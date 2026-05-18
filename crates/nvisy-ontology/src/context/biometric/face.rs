//! Face biometric reference data.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::entity::ContentSource;
use crate::primitive::BoundingBox;

/// Reference face data for identity matching.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct FaceData {
    /// Source pointer to the reference face image.
    pub image_source: ContentSource,
    /// Bounding box of the face within the image.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub region: Option<BoundingBox>,
    /// Base64-encoded face embedding / template.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub template: Option<String>,
    /// Algorithm that produced the template (e.g. `"arcface"`, `"facenet"`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub algorithm: Option<String>,
}

impl FaceData {
    /// Create face data pointing at a source image.
    pub fn new(image_source: ContentSource) -> Self {
        Self {
            image_source,
            region: None,
            template: None,
            algorithm: None,
        }
    }

    /// Set the face bounding box.
    pub fn with_region(mut self, region: BoundingBox) -> Self {
        self.region = Some(region);
        self
    }

    /// Set the encoded face template.
    pub fn with_template(mut self, template: impl Into<String>) -> Self {
        self.template = Some(template.into());
        self
    }

    /// Set the extraction algorithm.
    pub fn with_algorithm(mut self, algorithm: impl Into<String>) -> Self {
        self.algorithm = Some(algorithm.into());
        self
    }
}
