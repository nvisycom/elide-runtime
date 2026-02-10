use serde::{Deserialize, Serialize};
use uuid::Uuid;
use crate::types::Metadata;
use crate::datatypes::{
    entity::Entity, redaction::Redaction, policy::Policy, audit::Audit,
    document::Document, blob::Blob, image::ImageData,
};

/// Common fields shared by all domain data items.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct DataItem {
    pub id: Uuid,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parent_id: Option<Uuid>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<Metadata>,
}

impl DataItem {
    pub fn new() -> Self {
        Self {
            id: Uuid::new_v4(),
            parent_id: None,
            metadata: None,
        }
    }

    pub fn with_metadata(mut self, metadata: Metadata) -> Self {
        self.metadata = Some(metadata);
        self
    }

    pub fn derive_from(mut self, parent: &DataItem) -> Self {
        self.parent_id = Some(parent.id);
        self
    }
}

impl Default for DataItem {
    fn default() -> Self {
        Self::new()
    }
}

/// Discriminated union of all data types that flow through DAG channels.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(tag = "_type", rename_all = "snake_case")]
pub enum DataValue {
    Document(Document),
    Blob(Blob),
    Entity(Entity),
    Redaction(Redaction),
    Policy(Policy),
    Audit(Audit),
    Image(ImageData),
}

impl DataValue {
    /// Get the type name of this data value.
    pub fn type_name(&self) -> &'static str {
        match self {
            DataValue::Document(_) => "document",
            DataValue::Blob(_) => "blob",
            DataValue::Entity(_) => "entity",
            DataValue::Redaction(_) => "redaction",
            DataValue::Policy(_) => "policy",
            DataValue::Audit(_) => "audit",
            DataValue::Image(_) => "image",
        }
    }

    /// Get the underlying DataItem common fields.
    pub fn data_item(&self) -> &DataItem {
        match self {
            DataValue::Document(d) => &d.data,
            DataValue::Blob(b) => &b.data,
            DataValue::Entity(e) => &e.data,
            DataValue::Redaction(r) => &r.data,
            DataValue::Policy(p) => &p.data,
            DataValue::Audit(a) => &a.data,
            DataValue::Image(i) => &i.data,
        }
    }
}
