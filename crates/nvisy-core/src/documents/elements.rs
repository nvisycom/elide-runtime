use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::documents::ontology::ElementType;
use crate::types::Metadata;

/// An inline hyperlink within element text.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Link {
    pub text: String,
    pub url: String,
    pub start_index: usize,
}

/// An inline formatting span within element text.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmphasizedText {
    pub text: String,
    pub tag: String,
}

/// A single cell within a table structure.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TableCellData {
    pub row: usize,
    pub column: usize,
    pub text: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub is_header: Option<bool>,
}

/// Extraction / OCR provenance data.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ElementProvenance {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub confidence: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detection_origin: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub is_continuation: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub header_footer_type: Option<String>,
}

/// Structured key-value pair from a form.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FormKeyValuePair {
    pub key: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub value: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub confidence: Option<f64>,
}

/// A single structural element extracted from a document.
///
/// Combines base element fields with optional type-specific fields
/// (image, table, form, email) in a flat struct rather than inheritance.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Element {
    pub id: Uuid,
    #[serde(rename = "type")]
    pub element_type: ElementType,
    pub text: String,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub parent_id: Option<Uuid>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub page_number: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub page_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub level: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub languages: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<Metadata>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_tag: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text_as_html: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub links: Option<Vec<Link>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub emphasized_texts: Option<Vec<EmphasizedText>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub provenance: Option<ElementProvenance>,

    // Image-specific fields (when element_type is Image)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub image_base64: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub image_mime_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub image_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub image_path: Option<String>,

    // Table-specific fields (when element_type is Table)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cells: Option<Vec<TableCellData>>,

    // Form-specific fields (when element_type is Checkbox/FormKeysValues)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub checked: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub value: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub key_value_pairs: Option<Vec<FormKeyValuePair>>,

    // Email-specific fields (when element_type is EmailMessage)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sent_from: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sent_to: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cc_recipient: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bcc_recipient: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub subject: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub signature: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub email_message_id: Option<String>,
}

impl Element {
    pub fn new(element_type: ElementType, text: impl Into<String>) -> Self {
        Self {
            id: Uuid::new_v4(),
            element_type,
            text: text.into(),
            parent_id: None,
            page_number: None,
            page_name: None,
            level: None,
            languages: None,
            metadata: None,
            source_tag: None,
            text_as_html: None,
            links: None,
            emphasized_texts: None,
            provenance: None,
            image_base64: None,
            image_mime_type: None,
            image_url: None,
            image_path: None,
            cells: None,
            checked: None,
            value: None,
            key_value_pairs: None,
            sent_from: None,
            sent_to: None,
            cc_recipient: None,
            bcc_recipient: None,
            subject: None,
            signature: None,
            email_message_id: None,
        }
    }

    pub fn with_page_number(mut self, page: u32) -> Self {
        self.page_number = Some(page);
        self
    }

    pub fn with_level(mut self, level: u32) -> Self {
        self.level = Some(level);
        self
    }

    pub fn with_languages(mut self, langs: Vec<String>) -> Self {
        self.languages = Some(langs);
        self
    }
}
