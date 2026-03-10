//! Document template reference data.

use nvisy_core::math::BoundingBox;
use nvisy_core::content::ContentSource;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Reference document template for layout/type classification.
///
/// Used to detect documents of a known type (ID cards, passports, forms,
/// invoices) by comparing their visual layout against this reference.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct TemplateData {
    /// Source pointer to the reference template image.
    pub image_source: ContentSource,
    /// Optional sub-region of interest within the template.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub region: Option<BoundingBox>,
    /// Image format hint (e.g. `"jpeg"`, `"png"`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub format: Option<String>,
    /// Document type label (e.g. `"passport"`, `"drivers_license"`, `"invoice"`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub document_type: Option<String>,
}

impl TemplateData {
    /// Create template data pointing at a source image.
    pub fn new(image_source: ContentSource) -> Self {
        Self {
            image_source,
            region: None,
            format: None,
            document_type: None,
        }
    }

    /// Set the document type label.
    pub fn with_document_type(mut self, document_type: impl Into<String>) -> Self {
        self.document_type = Some(document_type.into());
        self
    }
}
