//! Document template reference data.

use elide_core::primitive::BoundingBox;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Reference document template for layout/type classification.
///
/// Used to detect documents of a known type (ID cards, passports, forms,
/// invoices) by comparing their visual layout against this reference.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct TemplateData {
    /// Id of the file holding the reference template image.
    pub image_source: Uuid,
    /// Optional sub-region of interest within the template.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub region: Option<BoundingBox>,
    /// Document type label (e.g. `"passport"`, `"drivers_license"`, `"invoice"`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub document_type: Option<String>,
}

impl TemplateData {
    /// Create template data pointing at a source image.
    pub fn new(image_source: Uuid) -> Self {
        Self {
            image_source,
            region: None,
            document_type: None,
        }
    }

    /// Set the document type label.
    pub fn with_document_type(mut self, document_type: impl Into<String>) -> Self {
        self.document_type = Some(document_type.into());
        self
    }
}
