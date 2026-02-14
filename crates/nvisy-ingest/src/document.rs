//! Unified document representation for any handleable content.

use bytes::Bytes;
use nvisy_core::path::ContentSource;
use serde::Serialize;

use crate::element::Element;
use crate::handler::{FormatHandler, Handler};

/// A unified representation of any content that can be handled by the pipeline.
///
/// `Document` is generic over `H`, a [`Handler`] that describes the source
/// format. For heterogeneous collections, use `Document<FormatHandler>`.
///
/// Fields are grouped by content modality:
/// - **Text** (`content`, `title`, `elements`, `page_count`) — for PDF, DOCX, HTML, etc.
/// - **Binary/image** (`data`, `mime_type`, `width`, `height`, etc.) — for images and raw bytes.
/// - **Tabular** (`columns`, `rows`, `sheet_name`) — for CSV, XLSX.
#[derive(Debug, Clone)]
pub struct Document<H: Handler> {
    /// Content source identity and lineage.
    pub source: ContentSource,

    // -- Text content (from text, PDF, DOCX, HTML, etc.) --

    /// Full text content, if applicable.
    pub content: Option<String>,
    /// Document title, if extracted.
    pub title: Option<String>,
    /// Structural elements parsed from the document.
    pub elements: Option<Vec<Element>>,
    /// Total page count for paginated formats.
    pub page_count: Option<u32>,

    // -- Binary/image content --

    /// Raw binary data (image bytes, audio bytes, etc.).
    pub data: Option<Bytes>,
    /// MIME type of the data (e.g. `"image/png"`, `"audio/wav"`).
    pub mime_type: Option<String>,
    /// Width in pixels (images).
    pub width: Option<u32>,
    /// Height in pixels (images).
    pub height: Option<u32>,
    /// File path or URL the content was loaded from.
    pub source_path: Option<String>,
    /// 1-based page number this was extracted from.
    pub page_number: Option<u32>,

    // -- Tabular content --

    /// Column header names.
    pub columns: Option<Vec<String>>,
    /// Row data (each inner Vec same length as columns).
    pub rows: Option<Vec<Vec<String>>>,
    /// Sheet or tab name within a multi-sheet workbook.
    pub sheet_name: Option<String>,

    /// Format handler (not serialized).
    handler: H,
}

impl<H: Handler> Document<H> {
    /// Create a new empty document with the given handler.
    pub fn new(handler: H) -> Self {
        Self {
            source: ContentSource::new(),
            content: None,
            title: None,
            elements: None,
            page_count: None,
            data: None,
            mime_type: None,
            width: None,
            height: None,
            source_path: None,
            page_number: None,
            columns: None,
            rows: None,
            sheet_name: None,
            handler,
        }
    }

    /// Get a reference to the format handler.
    pub fn handler(&self) -> &H {
        &self.handler
    }

    /// Original file format identifier (delegates to `handler.id()`).
    pub fn source_format(&self) -> &str {
        self.handler.id()
    }

    // -- Builder methods --

    /// Set text content.
    pub fn with_text(mut self, content: impl Into<String>) -> Self {
        self.content = Some(content.into());
        self
    }

    /// Set binary data and MIME type.
    pub fn with_data(mut self, data: impl Into<Bytes>, mime: impl Into<String>) -> Self {
        self.data = Some(data.into());
        self.mime_type = Some(mime.into());
        self
    }

    /// Set tabular content (columns + rows).
    pub fn with_tabular(mut self, columns: Vec<String>, rows: Vec<Vec<String>>) -> Self {
        self.columns = Some(columns);
        self.rows = Some(rows);
        self
    }

    /// Set the document title.
    pub fn with_title(mut self, title: impl Into<String>) -> Self {
        self.title = Some(title.into());
        self
    }

    /// Attach parsed structural elements.
    pub fn with_elements(mut self, elements: Vec<Element>) -> Self {
        self.elements = Some(elements);
        self
    }

    /// Set the total page count.
    pub fn with_page_count(mut self, count: u32) -> Self {
        self.page_count = Some(count);
        self
    }

    /// Set pixel dimensions (images).
    pub fn with_dimensions(mut self, width: u32, height: u32) -> Self {
        self.width = Some(width);
        self.height = Some(height);
        self
    }

    /// Set the source file path or URL.
    pub fn with_source_path(mut self, path: impl Into<String>) -> Self {
        self.source_path = Some(path.into());
        self
    }

    /// Set the 1-based page number this was extracted from.
    pub fn with_page_number(mut self, page: u32) -> Self {
        self.page_number = Some(page);
        self
    }

    /// Set the sheet/tab name for tabular data.
    pub fn with_sheet_name(mut self, name: impl Into<String>) -> Self {
        self.sheet_name = Some(name.into());
        self
    }

    /// Convert into a `Document<FormatHandler>` by wrapping the handler.
    pub fn into_format(self) -> Document<FormatHandler>
    where
        H: Into<FormatHandler>,
    {
        Document {
            source: self.source,
            content: self.content,
            title: self.title,
            elements: self.elements,
            page_count: self.page_count,
            data: self.data,
            mime_type: self.mime_type,
            width: self.width,
            height: self.height,
            source_path: self.source_path,
            page_number: self.page_number,
            columns: self.columns,
            rows: self.rows,
            sheet_name: self.sheet_name,
            handler: self.handler.into(),
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

    /// Create a Document by deriving content from element texts joined with "\n\n".
    pub fn from_elements(elements: Vec<Element>, handler: H) -> Self {
        let content = elements
            .iter()
            .map(|e| e.text.as_str())
            .collect::<Vec<_>>()
            .join("\n\n");
        let mut doc = Self::new(handler);
        doc.content = Some(content);
        doc.elements = Some(elements);
        doc
    }
}

// ---------------------------------------------------------------------------
// Serialization
// ---------------------------------------------------------------------------

impl<H: Handler> Serialize for Document<H> {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        use serde::ser::SerializeStruct;

        // Count always-present fields
        let mut count = 3; // id + parent_id + source_format
        if self.content.is_some() { count += 1; }
        if self.title.is_some() { count += 1; }
        if self.elements.is_some() { count += 1; }
        if self.page_count.is_some() { count += 1; }
        if self.data.is_some() { count += 1; }
        if self.mime_type.is_some() { count += 1; }
        if self.width.is_some() { count += 1; }
        if self.height.is_some() { count += 1; }
        if self.source_path.is_some() { count += 1; }
        if self.page_number.is_some() { count += 1; }
        if self.columns.is_some() { count += 1; }
        if self.rows.is_some() { count += 1; }
        if self.sheet_name.is_some() { count += 1; }

        let mut state = serializer.serialize_struct("Document", count)?;
        state.serialize_field("id", &self.source.as_uuid())?;
        state.serialize_field("parent_id", &self.source.parent_id())?;
        state.serialize_field("source_format", self.handler.id())?;

        if let Some(ref content) = self.content {
            state.serialize_field("content", content)?;
        }
        if let Some(ref title) = self.title {
            state.serialize_field("title", title)?;
        }
        if let Some(ref elements) = self.elements {
            state.serialize_field("elements", elements)?;
        }
        if let Some(page_count) = self.page_count {
            state.serialize_field("page_count", &page_count)?;
        }
        if let Some(ref data) = self.data {
            state.serialize_field("data", data.as_ref())?;
        }
        if let Some(ref mime_type) = self.mime_type {
            state.serialize_field("mime_type", mime_type)?;
        }
        if let Some(width) = self.width {
            state.serialize_field("width", &width)?;
        }
        if let Some(height) = self.height {
            state.serialize_field("height", &height)?;
        }
        if let Some(ref source_path) = self.source_path {
            state.serialize_field("source_path", source_path)?;
        }
        if let Some(page_number) = self.page_number {
            state.serialize_field("page_number", &page_number)?;
        }
        if let Some(ref columns) = self.columns {
            state.serialize_field("columns", columns)?;
        }
        if let Some(ref rows) = self.rows {
            state.serialize_field("rows", rows)?;
        }
        if let Some(ref sheet_name) = self.sheet_name {
            state.serialize_field("sheet_name", sheet_name)?;
        }

        state.end()
    }
}
