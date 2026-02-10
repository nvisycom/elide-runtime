//! Parsed document representation, structural elements, and element ontology.

use bytes::Bytes;
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use super::Data;
use super::Metadata;

// ---------------------------------------------------------------------------
// Element ontology
// ---------------------------------------------------------------------------

/// Broad grouping of element types.
///
/// Every [`ElementType`] belongs to exactly one category, providing
/// a coarse filter for pipeline actions that only operate on certain
/// kinds of content.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum ElementCategory {
    /// Narrative text, headings, list items, captions, and addresses.
    Text,
    /// Tabular data.
    Table,
    /// Images and other media content.
    Media,
    /// Source code fragments.
    Code,
    /// Mathematical formulae.
    Math,
    /// Form elements such as checkboxes and key-value fields.
    Form,
    /// Layout markers like page breaks and page numbers.
    Layout,
    /// Email message content.
    Email,
}

/// Specific structural element type extracted from a document.
///
/// Each variant maps to a single [`ElementCategory`] via
/// [`ElementType::category`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "kebab-case")]
pub enum ElementType {
    // -- Text --

    /// A document title or section heading.
    Title,
    /// A block of narrative prose.
    NarrativeText,
    /// An item within a bulleted or numbered list.
    ListItem,
    /// A page or section header.
    Header,
    /// A page or section footer.
    Footer,
    /// Caption text associated with a figure.
    FigureCaption,
    /// A physical or mailing address.
    Address,
    /// Text that does not fit any other text category.
    UncategorizedText,

    // -- Table --

    /// A data table with rows and columns.
    Table,

    // -- Media --

    /// An embedded image.
    Image,

    // -- Code --

    /// A source code snippet or block.
    CodeSnippet,

    // -- Math --

    /// A mathematical formula or equation.
    Formula,

    // -- Form --

    /// A checkbox form control.
    Checkbox,
    /// A set of key-value pairs extracted from a form.
    FormKeysValues,

    // -- Layout --

    /// A page break marker.
    PageBreak,
    /// A page number indicator.
    PageNumber,

    // -- Email --

    /// An email message body and headers.
    EmailMessage,
}

impl ElementType {
    /// Return the category this element type belongs to.
    pub fn category(&self) -> ElementCategory {
        match self {
            Self::Title
            | Self::NarrativeText
            | Self::ListItem
            | Self::Header
            | Self::Footer
            | Self::FigureCaption
            | Self::Address
            | Self::UncategorizedText => ElementCategory::Text,
            Self::Table => ElementCategory::Table,
            Self::Image => ElementCategory::Media,
            Self::CodeSnippet => ElementCategory::Code,
            Self::Formula => ElementCategory::Math,
            Self::Checkbox | Self::FormKeysValues => ElementCategory::Form,
            Self::PageBreak | Self::PageNumber => ElementCategory::Layout,
            Self::EmailMessage => ElementCategory::Email,
        }
    }
}

/// Parse an element type string and return its category.
///
/// Returns `None` if the string does not match any known [`ElementType`].
pub fn category_of(type_str: &str) -> Option<ElementCategory> {
    let et: ElementType =
        serde_json::from_value(serde_json::Value::String(type_str.to_string())).ok()?;
    Some(et.category())
}

// ---------------------------------------------------------------------------
// Structural elements
// ---------------------------------------------------------------------------

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

// ---------------------------------------------------------------------------
// Document
// ---------------------------------------------------------------------------

/// A parsed human-readable text representation of a document.
///
/// Documents are produced by loaders from raw blobs and contain the
/// extracted text along with optional structural elements, title, and
/// source format metadata.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct Document {
    /// Common data-item fields (id, parent_id, metadata).
    #[serde(flatten)]
    pub data: Data,
    /// Full text content of the document.
    pub content: String,
    /// Document title, if one was extracted.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    /// Structural elements (paragraphs, tables, images, etc.) parsed from the document.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub elements: Option<Vec<Element>>,
    /// Original file format (e.g. `"pdf"`, `"docx"`, `"html"`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_format: Option<String>,
    /// Total number of pages, if the source format is paginated.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub page_count: Option<u32>,
}

impl Document {
    /// Create a new document from raw text content.
    pub fn new(content: impl Into<String>) -> Self {
        Self {
            data: Data::new(),
            content: content.into(),
            title: None,
            elements: None,
            source_format: None,
            page_count: None,
        }
    }

    /// Set the document title (builder pattern).
    pub fn with_title(mut self, title: impl Into<String>) -> Self {
        self.title = Some(title.into());
        self
    }

    /// Attach parsed structural elements to this document.
    pub fn with_elements(mut self, elements: Vec<Element>) -> Self {
        self.elements = Some(elements);
        self
    }

    /// Record the original file format (e.g. `"pdf"`, `"docx"`).
    pub fn with_source_format(mut self, format: impl Into<String>) -> Self {
        self.source_format = Some(format.into());
        self
    }

    /// Set the total page count for paginated source formats.
    pub fn with_page_count(mut self, count: u32) -> Self {
        self.page_count = Some(count);
        self
    }

    /// Create a Document by deriving content from element texts joined with "\n\n".
    pub fn from_elements(elements: Vec<Element>) -> Self {
        let content = elements.iter().map(|e| e.text.as_str()).collect::<Vec<_>>().join("\n\n");
        Self {
            data: Data::new(),
            content,
            title: None,
            elements: Some(elements),
            source_format: None,
            page_count: None,
        }
    }

    /// Unique BCP-47 language tags collected from all elements.
    pub fn languages(&self) -> Vec<String> {
        let mut langs = Vec::new();
        if let Some(elements) = &self.elements {
            for el in elements {
                if let Some(ref element_langs) = el.languages {
                    for lang in element_langs {
                        if !langs.contains(lang) {
                            langs.push(lang.clone());
                        }
                    }
                }
            }
        }
        langs
    }

    /// Group elements by their 1-based page number.
    /// Elements without a page_number are collected under key 0.
    pub fn get_elements_by_page(&self) -> std::collections::HashMap<u32, Vec<&Element>> {
        let mut map = std::collections::HashMap::new();
        if let Some(elements) = &self.elements {
            for el in elements {
                let page = el.page_number.unwrap_or(0);
                map.entry(page).or_insert_with(Vec::new).push(el);
            }
        }
        map
    }
}

// ---------------------------------------------------------------------------
// ImageData
// ---------------------------------------------------------------------------

/// An image extracted from a document or provided directly.
///
/// Carries the raw pixel data, MIME type, optional dimensions, and
/// provenance information linking back to its source.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct ImageData {
    /// Common data-item fields (id, parent_id, metadata).
    #[serde(flatten)]
    pub data: Data,
    /// Raw image bytes (PNG, JPEG, etc.).
    #[serde(with = "crate::datatypes::blob::bytes_serde")]
    #[cfg_attr(feature = "schema", schemars(with = "Vec<u8>"))]
    pub image_data: Bytes,
    /// MIME type of the image (e.g. `"image/png"`).
    pub mime_type: String,
    /// Width of the image in pixels, if known.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub width: Option<u32>,
    /// Height of the image in pixels, if known.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub height: Option<u32>,
    /// File path or URL the image was loaded from, if applicable.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_path: Option<String>,
    /// 1-based page number the image was extracted from, if applicable.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub page_number: Option<u32>,
}

impl ImageData {
    /// Create a new image from raw bytes and a MIME type.
    pub fn new(image_data: impl Into<Bytes>, mime_type: impl Into<String>) -> Self {
        Self {
            data: Data::new(),
            image_data: image_data.into(),
            mime_type: mime_type.into(),
            width: None,
            height: None,
            source_path: None,
            page_number: None,
        }
    }

    /// Set the pixel dimensions of the image.
    pub fn with_dimensions(mut self, width: u32, height: u32) -> Self {
        self.width = Some(width);
        self.height = Some(height);
        self
    }

    /// Record the file path or URL the image originated from.
    pub fn with_source_path(mut self, path: impl Into<String>) -> Self {
        self.source_path = Some(path.into());
        self
    }

    /// Set the page number this image was extracted from.
    pub fn with_page_number(mut self, page: u32) -> Self {
        self.page_number = Some(page);
        self
    }
}
