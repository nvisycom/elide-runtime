//! Structural elements extracted from parsed documents.

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::documents::ontology::ElementType;
use crate::datatypes::Metadata;

/// An inline hyperlink within element text.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct Link {
    /// Display text of the hyperlink.
    pub text: String,
    /// Target URL of the hyperlink.
    pub url: String,
    /// Character offset where the link text begins in the parent element.
    pub start_index: usize,
}

/// An inline formatting span within element text.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct EmphasizedText {
    /// The emphasized text content.
    pub text: String,
    /// HTML tag name describing the emphasis (e.g. `"b"`, `"i"`, `"em"`).
    pub tag: String,
}

/// A single cell within a table structure.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct TableCellData {
    /// Zero-based row index.
    pub row: usize,
    /// Zero-based column index.
    pub column: usize,
    /// Text content of the cell.
    pub text: String,
    /// Whether this cell is a header cell.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub is_header: Option<bool>,
}

/// Extraction or OCR provenance data for an element.
///
/// Records how an element was detected and any extraction
/// confidence metadata.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct ElementProvenance {
    /// Confidence score of the extraction (0.0 to 1.0).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub confidence: Option<f64>,
    /// Name of the extraction engine or model that produced this element.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detection_origin: Option<String>,
    /// Whether this element continues from a previous element split across pages.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub is_continuation: Option<bool>,
    /// Type of header or footer (e.g. `"primary"`, `"footnote"`), if applicable.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub header_footer_type: Option<String>,
}

/// Structured key-value pair extracted from a form.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct FormKeyValuePair {
    /// Form field label or key.
    pub key: String,
    /// Form field value, if one was extracted.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub value: Option<String>,
    /// Extraction confidence for this key-value pair.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub confidence: Option<f64>,
}

/// A single structural element extracted from a document.
///
/// Combines base element fields with optional type-specific fields
/// (image, table, form, email) in a flat struct rather than inheritance.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct Element {
    /// Unique identifier for this element.
    pub id: Uuid,
    /// The structural type of this element.
    #[serde(rename = "type")]
    pub element_type: ElementType,
    /// Plain-text content of the element.
    pub text: String,

    /// Identifier of the parent element (for nested structures).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parent_id: Option<Uuid>,
    /// 1-based page number where this element appears.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub page_number: Option<u32>,
    /// Named page or sheet label (e.g. worksheet name in a spreadsheet).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub page_name: Option<String>,
    /// Heading level (1-6) for title or header elements.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub level: Option<u32>,
    /// BCP-47 language tags detected in this element.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub languages: Option<Vec<String>>,
    /// Arbitrary metadata associated with this element.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<Metadata>,
    /// Tag identifying the extraction source or pipeline stage.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_tag: Option<String>,
    /// HTML representation of the element's text with inline formatting.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text_as_html: Option<String>,
    /// Inline hyperlinks found within this element's text.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub links: Option<Vec<Link>>,
    /// Inline formatting spans (bold, italic, etc.) within this element.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub emphasized_texts: Option<Vec<EmphasizedText>>,
    /// Extraction or OCR provenance information.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub provenance: Option<ElementProvenance>,

    // -- Image-specific fields (when element_type is Image) --

    /// Base64-encoded image data.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub image_base64: Option<String>,
    /// MIME type of the embedded image.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub image_mime_type: Option<String>,
    /// Remote URL of the image.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub image_url: Option<String>,
    /// Local file path of the image.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub image_path: Option<String>,

    // -- Table-specific fields (when element_type is Table) --

    /// Individual table cells with row/column coordinates.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cells: Option<Vec<TableCellData>>,

    // -- Form-specific fields (when element_type is Checkbox/FormKeysValues) --

    /// Whether a checkbox is checked.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub checked: Option<bool>,
    /// Value of a form field.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub value: Option<String>,
    /// Structured key-value pairs extracted from a form.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub key_value_pairs: Option<Vec<FormKeyValuePair>>,

    // -- Email-specific fields (when element_type is EmailMessage) --

    /// Sender addresses.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sent_from: Option<Vec<String>>,
    /// Primary recipient addresses.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sent_to: Option<Vec<String>>,
    /// CC recipient addresses.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cc_recipient: Option<Vec<String>>,
    /// BCC recipient addresses.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bcc_recipient: Option<Vec<String>>,
    /// Email subject line.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub subject: Option<String>,
    /// Email signature block.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub signature: Option<String>,
    /// RFC 2822 Message-ID of the email.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub email_message_id: Option<String>,
}

impl Element {
    /// Create a new element with the given type and text content.
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

    /// Set the 1-based page number for this element.
    pub fn with_page_number(mut self, page: u32) -> Self {
        self.page_number = Some(page);
        self
    }

    /// Set the heading level (1-6) for title or header elements.
    pub fn with_level(mut self, level: u32) -> Self {
        self.level = Some(level);
        self
    }

    /// Set BCP-47 language tags detected in this element.
    pub fn with_languages(mut self, langs: Vec<String>) -> Self {
        self.languages = Some(langs);
        self
    }
}
