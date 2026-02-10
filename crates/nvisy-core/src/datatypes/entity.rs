use serde::{Deserialize, Serialize};
use uuid::Uuid;
use crate::data::DataItem;
use crate::types::{DetectionMethod, EntityCategory};

/// Bounding box for image-based entity locations.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BoundingBox {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

/// Location of an entity within its source document.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EntityLocation {
    pub start_offset: usize,
    pub end_offset: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub element_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub page_number: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bounding_box: Option<BoundingBox>,
}

/// A detected sensitive data occurrence within a document.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Entity {
    #[serde(flatten)]
    pub data: DataItem,
    pub category: EntityCategory,
    pub entity_type: String,
    pub value: String,
    pub detection_method: DetectionMethod,
    pub confidence: f64,
    pub location: EntityLocation,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_id: Option<Uuid>,
}

impl Entity {
    pub fn new(
        category: EntityCategory,
        entity_type: impl Into<String>,
        value: impl Into<String>,
        detection_method: DetectionMethod,
        confidence: f64,
        location: EntityLocation,
    ) -> Self {
        Self {
            data: DataItem::new(),
            category,
            entity_type: entity_type.into(),
            value: value.into(),
            detection_method,
            confidence,
            location,
            source_id: None,
        }
    }

    pub fn with_source_id(mut self, source_id: Uuid) -> Self {
        self.source_id = Some(source_id);
        self
    }
}
