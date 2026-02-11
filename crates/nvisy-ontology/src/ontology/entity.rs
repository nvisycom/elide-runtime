//! Sensitive-data entity types and detection metadata.

use serde::{Deserialize, Serialize};
use uuid::Uuid;
use nvisy_core::datatypes::Data;

/// Category of sensitive data an entity belongs to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum EntityCategory {
    /// Personally Identifiable Information (names, SSNs, addresses, etc.).
    Pii,
    /// Protected Health Information (HIPAA-regulated data).
    Phi,
    /// Financial data (credit card numbers, bank accounts, etc.).
    Financial,
    /// Secrets and credentials (API keys, passwords, tokens).
    Credentials,
    /// User-defined or plugin-specific category.
    Custom,
}

/// Method used to detect a sensitive entity.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum DetectionMethod {
    /// Regular expression pattern matching.
    Regex,
    /// Named-entity recognition via AI model.
    AiNer,
    /// Lookup in a known-value dictionary.
    Dictionary,
    /// Checksum or Luhn-algorithm validation.
    Checksum,
    /// Multiple methods combined to produce a single detection.
    Composite,
    /// OCR text extraction with bounding boxes.
    Ocr,
    /// User-provided annotations.
    Manual,
}

/// Axis-aligned bounding box for image-based entity locations.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct BoundingBox {
    /// Horizontal offset of the top-left corner (pixels or normalized).
    pub x: f64,
    /// Vertical offset of the top-left corner (pixels or normalized).
    pub y: f64,
    /// Width of the bounding box.
    pub width: f64,
    /// Height of the bounding box.
    pub height: f64,
}

/// Location of an entity within its source document or image.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct EntityLocation {
    /// Byte or character offset where the entity starts in the text.
    pub start_offset: usize,
    /// Byte or character offset where the entity ends in the text.
    pub end_offset: usize,
    /// Identifier of the document element containing this entity.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub element_id: Option<String>,
    /// 1-based page number where the entity was found.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub page_number: Option<u32>,
    /// Bounding box for image-based detections.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bounding_box: Option<BoundingBox>,
    /// Tabular row index (0-based).
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(default)]
    pub row_index: Option<usize>,
    /// Tabular column index (0-based).
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(default)]
    pub column_index: Option<usize>,
    /// Links this entity to a specific [`ImageData`](nvisy_core::datatypes::document::ImageData).
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(default)]
    pub image_id: Option<Uuid>,
}

/// A detected sensitive data occurrence within a document.
///
/// Entities are produced by detection actions (regex, NER, checksum, etc.)
/// and later consumed by redaction and audit actions.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct Entity {
    /// Common data-item fields (id, parent_id, metadata).
    #[serde(flatten)]
    pub data: Data,
    /// Broad classification of the sensitive data.
    pub category: EntityCategory,
    /// Specific type label (e.g. `"ssn"`, `"email"`, `"credit_card"`).
    pub entity_type: String,
    /// The matched text or value.
    pub value: String,
    /// How this entity was detected.
    pub detection_method: DetectionMethod,
    /// Detection confidence score in the range `[0.0, 1.0]`.
    pub confidence: f64,
    /// Where this entity was found in the source document.
    pub location: EntityLocation,
    /// Identifier of the source blob or document this entity came from.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_id: Option<Uuid>,
}

impl Entity {
    /// Create a new entity with the given detection details.
    pub fn new(
        category: EntityCategory,
        entity_type: impl Into<String>,
        value: impl Into<String>,
        detection_method: DetectionMethod,
        confidence: f64,
        location: EntityLocation,
    ) -> Self {
        Self {
            data: Data::new(),
            category,
            entity_type: entity_type.into(),
            value: value.into(),
            detection_method,
            confidence,
            location,
            source_id: None,
        }
    }

    /// Link this entity to the blob or document it was extracted from.
    pub fn with_source_id(mut self, source_id: Uuid) -> Self {
        self.source_id = Some(source_id);
        self
    }
}
