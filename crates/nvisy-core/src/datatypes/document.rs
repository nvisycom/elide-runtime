//! Parsed document representation.

use serde::{Deserialize, Serialize};
use super::DataItem;
use crate::documents::elements::Element;

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
    pub data: DataItem,
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
            data: DataItem::new(),
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
            data: DataItem::new(),
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
